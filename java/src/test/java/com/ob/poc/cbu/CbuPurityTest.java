package com.ob.poc.cbu;

import org.junit.jupiter.api.Test;
import java.io.IOException;
import java.nio.file.*;
import java.util.List;
import java.util.stream.Stream;
import static org.junit.jupiter.api.Assertions.fail;

public class CbuPurityTest {

    @Test
    public void testModelPackagePurity() throws IOException {
        Path modelPath = Paths.get("src/main/java/com/ob/poc/cbu/model");
        if (!Files.exists(modelPath)) {
            modelPath = Paths.get("../java/src/main/java/com/ob/poc/cbu/model");
        }
        
        if (!Files.exists(modelPath)) {
            fail("Model package path not found at: " + modelPath.toAbsolutePath());
        }

        try (Stream<Path> stream = Files.walk(modelPath)) {
            stream.filter(Files::isRegularFile)
                  .filter(path -> path.toString().endsWith(".java"))
                  .forEach(path -> {
                      try {
                          List<String> lines = Files.readAllLines(path);
                          for (int i = 0; i < lines.size(); i++) {
                              String line = lines.get(i);
                              String trimmed = line.trim();
                              
                              // Check imports
                              if (trimmed.startsWith("import ")) {
                                  if (trimmed.contains("org.jooq.") 
                                      || trimmed.contains("java.sql.") 
                                      || trimmed.contains("java.net.") 
                                      || trimmed.contains("com.ob.poc.cbu.db.") 
                                      || trimmed.contains("java.time.Clock")) {
                                      fail("Purity violation in " + path.getFileName() + " at line " + (i + 1) + 
                                           ": forbidden import '" + trimmed + "'");
                                  }
                              }
                              
                              // Check inline fully qualified names or references to forbidden types, ignoring comments
                              if (!trimmed.startsWith("//") && !trimmed.startsWith("*") && !trimmed.startsWith("/*")) {
                                  if (trimmed.contains("org.jooq.") 
                                      || trimmed.contains("java.sql.") 
                                      || trimmed.contains("java.net.") 
                                      || trimmed.contains("com.ob.poc.cbu.db.") 
                                      || trimmed.contains("java.time.Clock")) {
                                      fail("Purity violation in " + path.getFileName() + " at line " + (i + 1) + 
                                           ": forbidden reference in '" + trimmed + "'");
                                  }
                              }
                          }
                      } catch (IOException e) {
                          throw new RuntimeException(e);
                      }
                  });
        }
    }
}
