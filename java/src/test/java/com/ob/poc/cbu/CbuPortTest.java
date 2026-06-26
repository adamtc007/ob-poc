package com.ob.poc.cbu;

import com.ob.poc.cbu.model.*;
import com.ob.poc.cbu.db.*;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

import java.io.IOException;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.SQLException;
import java.sql.ResultSet;
import java.sql.Date;
import java.time.LocalDate;
import java.util.Collections;
import java.util.Set;
import java.util.UUID;
import java.util.List;
import com.fasterxml.jackson.databind.JsonNode;

public class CbuPortTest {

    private Connection getDbConnection() throws SQLException {
        String user = System.getProperty("user.name", "adamtc007");
        String url = "jdbc:postgresql://localhost:5432/data_designer";
        return DriverManager.getConnection(url, user, null);
    }

    private void executeRustDsl(String dsl) throws IOException, InterruptedException {
        String dbUrl = "postgresql://adamtc007@localhost:5432/data_designer";
        String[] cmd = {
            "cargo", "run",
            "--manifest-path", "../rust/Cargo.toml",
            "--bin", "dsl_cli",
            "--features", "cli",
            "--",
            "execute",
            "--db-url", dbUrl
        };
        ProcessBuilder pb = new ProcessBuilder(cmd);
        pb.redirectErrorStream(true);
        Process p = pb.start();
        
        try (var out = p.getOutputStream()) {
            out.write(dsl.getBytes());
            out.flush();
        }
        
        int exitCode = p.waitFor();
        if (exitCode != 0) {
            String output = new String(p.getInputStream().readAllBytes());
            throw new RuntimeException("Rust dsl_cli failed with exit code " + exitCode + "\nOutput:\n" + output);
        }
    }

    @Test
    public void testDecideSuspendSuccess() {
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));
        CbuCommand.Suspend cmd = new CbuCommand.Suspend(UUID.randomUUID(), actor, "Test suspension");

        CbuStatus status = new CbuStatus(
            cmd.id(),
            "Test CBU",
            new OperationalState.OperationallyActive(),
            new ValidationState.Validated(),
            new StructuralState.Structured(),
            new DispositionState.Active(),
            "FUND",
            "US"
        );

        DecisionOutcome outcome = CbuDecide.decide(cmd, status, null);
        assertTrue(outcome instanceof DecisionOutcome.Accept);
        DecisionOutcome.Accept accept = (DecisionOutcome.Accept) outcome;
        assertEquals(1, accept.effects().size());
        
        Effect.UpdateOperationalStatus effect = (Effect.UpdateOperationalStatus) accept.effects().get(0);
        assertEquals(cmd.id(), effect.cbuId());
        assertEquals("suspended", effect.toStatus());
        assertEquals("actively_trading", effect.fromStatus());
    }

    @Test
    public void testDecideSuspendWrongStateRefused() {
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));
        CbuCommand.Suspend cmd = new CbuCommand.Suspend(UUID.randomUUID(), actor, "Test suspension");

        // CBU already suspended
        CbuStatus status = new CbuStatus(
            cmd.id(),
            "Test CBU",
            new OperationalState.Suspended(),
            new ValidationState.Validated(),
            new StructuralState.Structured(),
            new DispositionState.Active(),
            "FUND",
            "US"
        );

        DecisionOutcome outcome = CbuDecide.decide(cmd, status, null);
        assertTrue(outcome instanceof DecisionOutcome.Refuse);
        DecisionOutcome.Refuse refuse = (DecisionOutcome.Refuse) outcome;
        assertTrue(refuse.reason().contains("must be in OperationallyActive state"));
    }

    @Test
    public void testDecideSuspendRoleDenied() {
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("analyst"));
        CbuCommand.Suspend cmd = new CbuCommand.Suspend(UUID.randomUUID(), actor, "Test suspension");

        CbuStatus status = new CbuStatus(
            cmd.id(),
            "Test CBU",
            new OperationalState.OperationallyActive(),
            new ValidationState.Validated(),
            new StructuralState.Structured(),
            new DispositionState.Active(),
            "FUND",
            "US"
        );

        DecisionOutcome outcome = CbuDecide.decide(cmd, status, null);
        assertTrue(outcome instanceof DecisionOutcome.Refuse);
        DecisionOutcome.Refuse refuse = (DecisionOutcome.Refuse) outcome;
        assertTrue(refuse.reason().contains("compliance roles"));
    }

    @Test
    public void testDecideSuspendValidationGateRefused() {
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));
        CbuCommand.Suspend cmd = new CbuCommand.Suspend(UUID.randomUUID(), actor, "Test suspension");

        // CBU is not validated (pre-validated status DISCOVERED)
        CbuStatus status = new CbuStatus(
            cmd.id(),
            "Test CBU",
            new OperationalState.OperationallyActive(),
            null, // validation status NULL
            new StructuralState.Discovered(),
            new DispositionState.Active(),
            "FUND",
            "US"
        );

        DecisionOutcome outcome = CbuDecide.decide(cmd, status, null);
        assertTrue(outcome instanceof DecisionOutcome.Refuse);
        DecisionOutcome.Refuse refuse = (DecisionOutcome.Refuse) outcome;
        assertTrue(refuse.reason().contains("CBU must be VALIDATED before operational status can be modified"));
    }

    @Test
    public void testDecideCreateSuccess() {
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("analyst"));
        UUID id = UUID.randomUUID();
        CbuCommand.Create cmd = new CbuCommand.Create(
            id,
            actor,
            "New Test CBU " + id,
            "US",
            null,
            null,
            "FUND",
            "Nature & Purpose",
            "Desc",
            null
        );

        DecisionOutcome outcome = CbuDecide.decide(cmd, null, new CbuDecide.DecisionContext(false));
        assertTrue(outcome instanceof DecisionOutcome.Accept);
        DecisionOutcome.Accept accept = (DecisionOutcome.Accept) outcome;
        assertEquals(2, accept.effects().size());
        assertTrue(accept.effects().get(0) instanceof Effect.InsertCbu);
        assertTrue(accept.effects().get(1) instanceof Effect.EmitPendingStateAdvance);
    }

    @Test
    public void testDecideCreateIdempotentByName() {
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("analyst"));
        UUID id = UUID.randomUUID();
        CbuCommand.Create cmd = new CbuCommand.Create(
            id,
            actor,
            "Existing Test CBU",
            "US",
            null,
            null,
            "FUND",
            "Nature & Purpose",
            "Desc",
            null
        );

        CbuStatus status = new CbuStatus(
            id,
            "Existing Test CBU",
            null,
            null,
            null,
            new DispositionState.Active(),
            "FUND",
            "US"
        );

        DecisionOutcome outcome = CbuDecide.decide(cmd, status, new CbuDecide.DecisionContext(false));
        assertTrue(outcome instanceof DecisionOutcome.Accept);
        DecisionOutcome.Accept accept = (DecisionOutcome.Accept) outcome;
        assertTrue(accept.effects().isEmpty());
    }

    @Test
    public void testLiveDbIntegration() throws SQLException {
        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // Test Create
                CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));
                UUID cbuId = UUID.randomUUID();
                String uniqueName = "Java Test CBU " + cbuId;
                
                CbuCommand.Create createCmd = new CbuCommand.Create(
                    cbuId,
                    actor,
                    uniqueName,
                    "IE",
                    null,
                    null,
                    "FUND",
                    "Testing mechanical port",
                    "A test CBU created via Java JDBC",
                    null
                );

                CbuExecutionResult result = CbuExecutor.execute(conn, createCmd);
                assertTrue(result instanceof CbuExecutionResult.Success, "Creation should succeed");
                CbuExecutionResult.Success success = (CbuExecutionResult.Success) result;
                assertTrue(success.created(), "CBU should be newly created");
                assertEquals(cbuId, success.cbuId());

                // Recover the CBU and assert attributes
                CbuStatus status = CbuRepository.recover(conn, cbuId);
                assertNotNull(status);
                assertEquals(uniqueName, status.name());
                assertEquals("IE", status.jurisdiction());
                assertTrue(status.disposition() instanceof DispositionState.Active);

                // Initially, status is DISCOVERED, so we manually update status to VALIDATED and operational_status to actively_trading to pass the gates
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'VALIDATED', operational_status = 'actively_trading' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    assertEquals(1, ps.executeUpdate());
                }

                // Verify recovered state reflects Validated and actively_trading
                status = CbuRepository.recover(conn, cbuId);
                assertTrue(status.op() instanceof OperationalState.OperationallyActive);
                assertTrue(status.val() instanceof ValidationState.Validated);

                // Test Suspend
                CbuCommand.Suspend suspendCmd = new CbuCommand.Suspend(cbuId, actor, "Live test suspension");
                result = CbuExecutor.execute(conn, suspendCmd);
                assertTrue(result instanceof CbuExecutionResult.Success, "Suspension should succeed");
                
                // Recover and verify operational status is suspended
                status = CbuRepository.recover(conn, cbuId);
                assertTrue(status.op() instanceof OperationalState.Suspended);

                // Test Read Projection (Track R)
                CbuDto dto = CbuReadProjection.read(conn, cbuId);
                assertNotNull(dto);
                assertEquals(uniqueName, dto.name());
                assertEquals("suspended", dto.operationalStatus());

            } finally {
                conn.rollback();
            }
        }
    }

    @Test
    public void testRustJavaDifferentialConformance() throws Exception {
        UUID cbuId = UUID.randomUUID();
        String uniqueName = "Diff Test CBU " + cbuId;
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));

        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // 1. Setup clean state
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE name = ?")) {
                    ps.setString(1, uniqueName);
                    ps.executeUpdate();
                }
                conn.commit();

                // 2. Execute Create in Rust
                String rustCreateDsl = "(cbu.create :name \"" + uniqueName + "\" :jurisdiction \"LU\" :client-type \"FUND\")";
                executeRustDsl(rustCreateDsl);

                // 3. Recover Rust state
                CbuStatus rustCreateStatus = CbuRepository.recoverByNameAndJurisdiction(conn, uniqueName, "LU");
                assertNotNull(rustCreateStatus, "Rust CBU creation must have occurred in DB");
                UUID rustCbuId = rustCreateStatus.id();

                // 4. Reset DB to Java execution state
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE name = ?")) {
                    ps.setString(1, uniqueName);
                    ps.executeUpdate();
                }
                conn.commit();

                // 5. Execute Create in Java
                CbuCommand.Create javaCreateCmd = new CbuCommand.Create(
                    rustCbuId,
                    actor,
                    uniqueName,
                    "LU",
                    null,
                    null,
                    "FUND",
                    null,
                    null,
                    null
                );
                CbuExecutionResult createRes = CbuExecutor.execute(conn, javaCreateCmd);
                assertTrue(createRes instanceof CbuExecutionResult.Success);
                
                CbuStatus javaCreateStatus = CbuRepository.recover(conn, rustCbuId);
                assertNotNull(javaCreateStatus);

                // 6. Diff Create Database State
                assertEquals(rustCreateStatus.name(), javaCreateStatus.name());
                assertEquals(rustCreateStatus.jurisdiction(), javaCreateStatus.jurisdiction());
                assertEquals(rustCreateStatus.clientType(), javaCreateStatus.clientType());
                assertEquals(rustCreateStatus.op().getClass(), javaCreateStatus.op().getClass(), "Operational state class should match (PreValidated)");
                assertEquals(rustCreateStatus.val(), javaCreateStatus.val(), "Validation state should match (null)");
                assertEquals(rustCreateStatus.struct().getClass(), javaCreateStatus.struct().getClass(), "Structural state class should match (Discovered)");
                assertEquals(rustCreateStatus.disposition().getClass(), javaCreateStatus.disposition().getClass(), "Disposition state class should match (Active)");

                // 7. Test Idempotency (Call Java Create twice)
                CbuExecutionResult createRes2 = CbuExecutor.execute(conn, javaCreateCmd);
                assertTrue(createRes2 instanceof CbuExecutionResult.Success);
                assertFalse(((CbuExecutionResult.Success) createRes2).created(), "Second call should not re-create");
                assertEquals(0, ((CbuExecutionResult.Success) createRes2).events().size(), "Second call should not generate side-effects");

                // 8. Prepare for Suspend Test (Advance state to Validated & OperationallyActive)
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'VALIDATED', operational_status = 'actively_trading' WHERE cbu_id = ?")) {
                    ps.setObject(1, rustCbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // 9. Execute Suspend in Rust
                String rustSuspendDsl = "(cbu.suspend :cbu-id \"" + rustCbuId + "\" :reason \"Rust diff suspension\")";
                executeRustDsl(rustSuspendDsl);

                // 10. Recover Rust Suspend state
                CbuStatus rustSuspendStatus = CbuRepository.recover(conn, rustCbuId);
                assertTrue(rustSuspendStatus.op() instanceof OperationalState.Suspended);

                // 11. Reset DB back to active state for Java Suspend test
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET operational_status = 'actively_trading' WHERE cbu_id = ?")) {
                    ps.setObject(1, rustCbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // 12. Execute Suspend in Java
                CbuCommand.Suspend javaSuspendCmd = new CbuCommand.Suspend(rustCbuId, actor, "Java diff suspension");
                CbuExecutionResult suspendRes = CbuExecutor.execute(conn, javaSuspendCmd);
                assertTrue(suspendRes instanceof CbuExecutionResult.Success);

                // 13. Diff Suspend Database State
                CbuStatus javaSuspendStatus = CbuRepository.recover(conn, rustCbuId);
                assertTrue(javaSuspendStatus.op() instanceof OperationalState.Suspended);
                assertEquals(rustSuspendStatus.op().getClass(), javaSuspendStatus.op().getClass(), "Operational state class should match (Suspended)");

                // 14. Diff Read Projection
                CbuDto rustDto = CbuReadProjection.read(conn, rustCbuId);
                assertNotNull(rustDto);
                assertEquals("suspended", rustDto.operationalStatus());

            } finally {
                // Cleanup test row completely
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE name = ?")) {
                    ps.setString(1, uniqueName);
                    ps.executeUpdate();
                }
                conn.commit();
            }
        }
    }

    @Test
    public void testDecideSubmitForValidation() {
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("analyst"));
        UUID id = UUID.randomUUID();
        CbuCommand.SubmitForValidation cmd = new CbuCommand.SubmitForValidation(id, actor);

        CbuStatus status = new CbuStatus(
            id, "Test CBU", new OperationalState.PreValidated(), null, new StructuralState.Discovered(), new DispositionState.Active(), "FUND", "LU",
            "DISCOVERED", null, "active"
        );

        DecisionOutcome outcome = CbuDecide.decide(cmd, status, null);
        assertTrue(outcome instanceof DecisionOutcome.Accept);
        DecisionOutcome.Accept accept = (DecisionOutcome.Accept) outcome;
        assertEquals(1, accept.effects().size());
        assertTrue(accept.effects().get(0) instanceof Effect.UpdateValidationStatus);
        Effect.UpdateValidationStatus e = (Effect.UpdateValidationStatus) accept.effects().get(0);
        assertEquals("DISCOVERED", e.fromStatus());
        assertEquals("VALIDATION_PENDING", e.toStatus());
    }

    @Test
    public void testDecideConfirmReject() {
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("analyst"));
        UUID id = UUID.randomUUID();
        CbuCommand.Confirm confirmCmd = new CbuCommand.Confirm(id, actor);
        CbuCommand.Reject rejectCmd = new CbuCommand.Reject(id, actor);

        CbuStatus statusPending = new CbuStatus(
            id, "Test CBU", new OperationalState.PreValidated(), new ValidationState.ValidationPending(), new StructuralState.Structured(), new DispositionState.Active(), "FUND", "LU",
            "VALIDATION_PENDING", null, "active"
        );

        DecisionOutcome outcomeConfirm = CbuDecide.decide(confirmCmd, statusPending, null);
        assertTrue(outcomeConfirm instanceof DecisionOutcome.Accept);
        Effect.UpdateValidationStatus eConf = (Effect.UpdateValidationStatus) ((DecisionOutcome.Accept) outcomeConfirm).effects().get(0);
        assertEquals("VALIDATED", eConf.toStatus());

        DecisionOutcome outcomeReject = CbuDecide.decide(rejectCmd, statusPending, null);
        assertTrue(outcomeReject instanceof DecisionOutcome.Accept);
        Effect.UpdateValidationStatus eRej = (Effect.UpdateValidationStatus) ((DecisionOutcome.Accept) outcomeReject).effects().get(0);
        assertEquals("VALIDATION_FAILED", eRej.toStatus());
    }

    @Test
    public void testDecideRestrictUnrestrict() {
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("analyst"));
        UUID id = UUID.randomUUID();
        CbuCommand.Restrict restrictCmd = new CbuCommand.Restrict(id, actor, "GLOBAL");
        CbuCommand.Unrestrict unrestrictCmd = new CbuCommand.Unrestrict(id, actor);

        CbuStatus statusActive = new CbuStatus(
            id, "Test CBU", new OperationalState.OperationallyActive(), new ValidationState.Validated(), new StructuralState.Structured(), new DispositionState.Active(), "FUND", "LU",
            "VALIDATED", "actively_trading", "active"
        );

        DecisionOutcome outcomeRestrict = CbuDecide.decide(restrictCmd, statusActive, null);
        assertTrue(outcomeRestrict instanceof DecisionOutcome.Accept);
        Effect.UpdateOperationalStatus eRest = (Effect.UpdateOperationalStatus) ((DecisionOutcome.Accept) outcomeRestrict).effects().get(0);
        assertEquals("restricted", eRest.toStatus());

        CbuStatus statusRestricted = new CbuStatus(
            id, "Test CBU", new OperationalState.Restricted(), new ValidationState.Validated(), new StructuralState.Structured(), new DispositionState.Active(), "FUND", "LU",
            "VALIDATED", "restricted", "active"
        );

        DecisionOutcome outcomeUnrestrict = CbuDecide.decide(unrestrictCmd, statusRestricted, null);
        assertTrue(outcomeUnrestrict instanceof DecisionOutcome.Accept);
        Effect.UpdateOperationalStatus eUnrest = (Effect.UpdateOperationalStatus) ((DecisionOutcome.Accept) outcomeUnrestrict).effects().get(0);
        assertEquals("actively_trading", eUnrest.toStatus());
    }

    @Test
    public void testDecideWindingDownOffboardRoles() {
        CbuCommand.Principal analyst = new CbuCommand.Principal("test-user", Set.of("analyst"));
        CbuCommand.Principal complianceAdmin = new CbuCommand.Principal("test-user", Set.of("compliance_admin"));
        UUID id = UUID.randomUUID();
        CbuCommand.BeginWindingDown windCmd = new CbuCommand.BeginWindingDown(id, complianceAdmin, "Close request");
        CbuCommand.BeginWindingDown windCmdAnalyst = new CbuCommand.BeginWindingDown(id, analyst, "Close request");

        CbuStatus statusActive = new CbuStatus(
            id, "Test CBU", new OperationalState.OperationallyActive(), new ValidationState.Validated(), new StructuralState.Structured(), new DispositionState.Active(), "FUND", "LU",
            "VALIDATED", "actively_trading", "active"
        );

        assertTrue(CbuDecide.decide(windCmdAnalyst, statusActive, null) instanceof DecisionOutcome.Refuse);
        
        DecisionOutcome outcomeWind = CbuDecide.decide(windCmd, statusActive, null);
        assertTrue(outcomeWind instanceof DecisionOutcome.Accept);
        Effect.UpdateOperationalStatus eWind = (Effect.UpdateOperationalStatus) ((DecisionOutcome.Accept) outcomeWind).effects().get(0);
        assertEquals("winding_down", eWind.toStatus());
    }

    @Test
    public void testRustJavaComprehensiveDifferentialConformance() throws Exception {
        UUID cbuId = UUID.randomUUID();
        String uniqueName = "Comp Diff Test CBU " + cbuId;
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer", "compliance_admin"));

        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // 1. Create a CBU in Java (genesis)
                CbuCommand.Create createCmd = new CbuCommand.Create(
                    cbuId, actor, uniqueName, "LU", null, null, "FUND", null, null, null
                );
                CbuExecutionResult res = CbuExecutor.execute(conn, createCmd);
                assertTrue(res instanceof CbuExecutionResult.Success);
                conn.commit();

                // Check initial recovered status
                CbuStatus initialStatus = CbuRepository.recover(conn, cbuId);
                assertNotNull(initialStatus);
                assertEquals("DISCOVERED", initialStatus.rawStatus());

                // --- TRANSITION 1: submit-for-validation ---
                // Rust
                executeRustDsl("(cbu.submit-for-validation :cbu-id \"" + cbuId + "\")");
                CbuStatus rustValidationPending = CbuRepository.recover(conn, cbuId);
                assertEquals("VALIDATION_PENDING", rustValidationPending.rawStatus());

                // Reset Java side DB state back to DISCOVERED
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'DISCOVERED' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java SubmitForValidation
                res = CbuExecutor.execute(conn, new CbuCommand.SubmitForValidation(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaValidationPending = CbuRepository.recover(conn, cbuId);
                assertEquals(rustValidationPending.rawStatus(), javaValidationPending.rawStatus());

                // --- TRANSITION 2: confirm (validation -> validated) ---
                // Rust
                executeRustDsl("(cbu.confirm :cbu-id \"" + cbuId + "\")");
                CbuStatus rustValidated = CbuRepository.recover(conn, cbuId);
                assertEquals("VALIDATED", rustValidated.rawStatus());

                // Reset Java side DB state back to VALIDATION_PENDING
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'VALIDATION_PENDING' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Confirm
                res = CbuExecutor.execute(conn, new CbuCommand.Confirm(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaValidated = CbuRepository.recover(conn, cbuId);
                assertEquals(rustValidated.rawStatus(), javaValidated.rawStatus());

                // Manually initialize operational status to actively_trading to test operational lifecycle (junction_state_from_primary: VALIDATED)
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET operational_status = 'actively_trading' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // --- TRANSITION 3: suspend ---
                // Rust
                executeRustDsl("(cbu.suspend :cbu-id \"" + cbuId + "\" :reason \"Susending\")");
                CbuStatus rustSuspended = CbuRepository.recover(conn, cbuId);
                assertEquals("suspended", rustSuspended.rawOpStatus());

                // Reset Java side operational_status back to actively_trading
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET operational_status = 'actively_trading' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Suspend
                res = CbuExecutor.execute(conn, new CbuCommand.Suspend(cbuId, actor, "Java Suspend"));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaSuspended = CbuRepository.recover(conn, cbuId);
                assertEquals(rustSuspended.rawOpStatus(), javaSuspended.rawOpStatus());

                // --- TRANSITION 4: reinstate ---
                // Rust
                executeRustDsl("(cbu.reinstate :cbu-id \"" + cbuId + "\")");
                CbuStatus rustReinstated = CbuRepository.recover(conn, cbuId);
                assertEquals("actively_trading", rustReinstated.rawOpStatus());

                // Reset Java side operational_status back to suspended
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET operational_status = 'suspended' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Reinstate
                res = CbuExecutor.execute(conn, new CbuCommand.Reinstate(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaReinstated = CbuRepository.recover(conn, cbuId);
                assertEquals(rustReinstated.rawOpStatus(), javaReinstated.rawOpStatus());

                // --- TRANSITION 5: restrict ---
                // Rust
                executeRustDsl("(cbu.restrict :cbu-id \"" + cbuId + "\" :scope \"US\")");
                CbuStatus rustRestricted = CbuRepository.recover(conn, cbuId);
                assertEquals("restricted", rustRestricted.rawOpStatus());

                // Reset Java side operational_status back to actively_trading
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET operational_status = 'actively_trading' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Restrict
                res = CbuExecutor.execute(conn, new CbuCommand.Restrict(cbuId, actor, "US"));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaRestricted = CbuRepository.recover(conn, cbuId);
                assertEquals(rustRestricted.rawOpStatus(), javaRestricted.rawOpStatus());

                // --- TRANSITION 6: unrestrict ---
                // Rust
                executeRustDsl("(cbu.unrestrict :cbu-id \"" + cbuId + "\")");
                CbuStatus rustUnrestricted = CbuRepository.recover(conn, cbuId);
                assertEquals("actively_trading", rustUnrestricted.rawOpStatus());

                // Reset Java side operational_status back to restricted
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET operational_status = 'restricted' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Unrestrict
                res = CbuExecutor.execute(conn, new CbuCommand.Unrestrict(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaUnrestricted = CbuRepository.recover(conn, cbuId);
                assertEquals(rustUnrestricted.rawOpStatus(), javaUnrestricted.rawOpStatus());

                // --- TRANSITION 7: flag-for-remediation ---
                // Rust
                executeRustDsl("(cbu.flag-for-remediation :cbu-id \"" + cbuId + "\" :reason \"Breach detected\")");
                CbuStatus rustRemediation = CbuRepository.recover(conn, cbuId);
                assertEquals("under_remediation", rustRemediation.rawDispStatus());

                // Reset Java side disposition_status back to active
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET disposition_status = 'active' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java FlagForRemediation
                res = CbuExecutor.execute(conn, new CbuCommand.FlagForRemediation(cbuId, actor, "Java Breach"));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaRemediation = CbuRepository.recover(conn, cbuId);
                assertEquals(rustRemediation.rawDispStatus(), javaRemediation.rawDispStatus());

                // --- TRANSITION 8: clear-remediation ---
                // Rust
                executeRustDsl("(cbu.clear-remediation :cbu-id \"" + cbuId + "\")");
                CbuStatus rustClear = CbuRepository.recover(conn, cbuId);
                assertEquals("active", rustClear.rawDispStatus());

                // Reset Java side disposition_status back to under_remediation
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET disposition_status = 'under_remediation' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java ClearRemediation
                res = CbuExecutor.execute(conn, new CbuCommand.ClearRemediation(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaClear = CbuRepository.recover(conn, cbuId);
                assertEquals(rustClear.rawDispStatus(), javaClear.rawDispStatus());

                // --- TRANSITION 9: request-proof-update ---
                // Rust
                executeRustDsl("(cbu.request-proof-update :cbu-id \"" + cbuId + "\")");
                CbuStatus rustProofUpdate = CbuRepository.recover(conn, cbuId);
                assertEquals("UPDATE_PENDING_PROOF", rustProofUpdate.rawStatus());

                // Reset Java side status back to VALIDATED
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'VALIDATED' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java RequestProofUpdate
                res = CbuExecutor.execute(conn, new CbuCommand.RequestProofUpdate(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaProofUpdate = CbuRepository.recover(conn, cbuId);
                assertEquals(rustProofUpdate.rawStatus(), javaProofUpdate.rawStatus());

                // --- TRANSITION 10: resubmit-proof ---
                // Rust
                executeRustDsl("(cbu.resubmit-proof :cbu-id \"" + cbuId + "\")");
                CbuStatus rustResubmitProof = CbuRepository.recover(conn, cbuId);
                assertEquals("VALIDATION_PENDING", rustResubmitProof.rawStatus());

                // Reset Java side status back to UPDATE_PENDING_PROOF
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'UPDATE_PENDING_PROOF' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java ResubmitProof
                res = CbuExecutor.execute(conn, new CbuCommand.ResubmitProof(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaResubmitProof = CbuRepository.recover(conn, cbuId);
                assertEquals(rustResubmitProof.rawStatus(), javaResubmitProof.rawStatus());

                // --- TRANSITION 11: reject ---
                // Rust
                executeRustDsl("(cbu.reject :cbu-id \"" + cbuId + "\")");
                CbuStatus rustRejected = CbuRepository.recover(conn, cbuId);
                assertEquals("VALIDATION_FAILED", rustRejected.rawStatus());

                // Reset Java side status back to VALIDATION_PENDING
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'VALIDATION_PENDING' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Reject
                res = CbuExecutor.execute(conn, new CbuCommand.Reject(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaRejected = CbuRepository.recover(conn, cbuId);
                assertEquals(rustRejected.rawStatus(), javaRejected.rawStatus());

                // --- TRANSITION 12: reopen-validation ---
                // Rust
                executeRustDsl("(cbu.reopen-validation :cbu-id \"" + cbuId + "\")");
                CbuStatus rustReopened = CbuRepository.recover(conn, cbuId);
                assertEquals("VALIDATION_PENDING", rustReopened.rawStatus());

                // Reset Java side status back to VALIDATION_FAILED
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'VALIDATION_FAILED' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java ReopenValidation
                res = CbuExecutor.execute(conn, new CbuCommand.ReopenValidation(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaReopened = CbuRepository.recover(conn, cbuId);
                assertEquals(rustReopened.rawStatus(), javaReopened.rawStatus());

                // Bring back to VALIDATED for operational winding down tests
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'VALIDATED' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // --- TRANSITION 13: begin-winding-down ---
                // Rust
                executeRustDsl("(cbu.begin-winding-down :cbu-id \"" + cbuId + "\" :reason \"Exit plan\")");
                CbuStatus rustWinding = CbuRepository.recover(conn, cbuId);
                assertEquals("winding_down", rustWinding.rawOpStatus());

                // Reset Java side operational_status back to actively_trading
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET operational_status = 'actively_trading' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java BeginWindingDown
                res = CbuExecutor.execute(conn, new CbuCommand.BeginWindingDown(cbuId, actor, "Exit plan"));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaWinding = CbuRepository.recover(conn, cbuId);
                assertEquals(rustWinding.rawOpStatus(), javaWinding.rawOpStatus());

                // --- TRANSITION 14: complete-offboard ---
                // Rust
                executeRustDsl("(cbu.complete-offboard :cbu-id \"" + cbuId + "\")");
                CbuStatus rustOffboarded = CbuRepository.recover(conn, cbuId);
                assertEquals("offboarded", rustOffboarded.rawOpStatus());

                // Reset Java side operational_status back to winding_down
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET operational_status = 'winding_down' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java CompleteOffboard
                res = CbuExecutor.execute(conn, new CbuCommand.CompleteOffboard(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaOffboarded = CbuRepository.recover(conn, cbuId);
                assertEquals(rustOffboarded.rawOpStatus(), javaOffboarded.rawOpStatus());

                // --- TRANSITION 15: soft-delete ---
                // Rust
                executeRustDsl("(cbu.soft-delete :cbu-id \"" + cbuId + "\" :reason \"Obsolete\")");
                CbuStatus rustSoftDeleted = CbuRepository.recover(conn, cbuId);
                assertEquals("soft_deleted", rustSoftDeleted.rawDispStatus());

                // Reset Java side disposition_status back to active
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET disposition_status = 'active' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java SoftDelete
                res = CbuExecutor.execute(conn, new CbuCommand.SoftDelete(cbuId, actor, "Obsolete"));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaSoftDeleted = CbuRepository.recover(conn, cbuId);
                assertEquals(rustSoftDeleted.rawDispStatus(), javaSoftDeleted.rawDispStatus());

                // --- TRANSITION 16: restore ---
                // Rust
                executeRustDsl("(cbu.restore :cbu-id \"" + cbuId + "\")");
                CbuStatus rustRestored = CbuRepository.recover(conn, cbuId);
                assertEquals("active", rustRestored.rawDispStatus());

                // Reset Java side disposition_status back to soft_deleted
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET disposition_status = 'soft_deleted' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Restore
                res = CbuExecutor.execute(conn, new CbuCommand.Restore(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaRestored = CbuRepository.recover(conn, cbuId);
                assertEquals(rustRestored.rawDispStatus(), javaRestored.rawDispStatus());

                // --- TRANSITION 17: hard-delete ---
                // Rust
                executeRustDsl("(cbu.hard-delete :cbu-id \"" + cbuId + "\")");
                CbuStatus rustHardDeleted = CbuRepository.recover(conn, cbuId);
                assertEquals("hard_deleted", rustHardDeleted.rawDispStatus());

                // Reset Java side disposition_status back to active
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET disposition_status = 'active' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java HardDelete
                res = CbuExecutor.execute(conn, new CbuCommand.HardDelete(cbuId, actor));
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuStatus javaHardDeleted = CbuRepository.recover(conn, cbuId);
                assertEquals(rustHardDeleted.rawDispStatus(), javaHardDeleted.rawDispStatus());

            } finally {
                // Cleanup test row completely
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();
            }
        }
    }

    @Test
    public void testReadProjectionsDifferentialConformance() throws Exception {
        UUID cbuId = UUID.randomUUID();
        UUID childCbuId = UUID.randomUUID();
        UUID entityId1 = UUID.randomUUID();
        UUID entityId2 = UUID.randomUUID();
        UUID docId = UUID.randomUUID();
        UUID contractId = UUID.randomUUID();
        UUID rateCardId = UUID.randomUUID();
        UUID evidenceId = UUID.randomUUID();
        String cbuName = "Test Read Proj CBU " + cbuId;
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));

        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // 1. Create parent and child CBUs
                String uniqueJurisdiction = "JR_" + cbuId.toString().substring(0, 8);
                String uniqueClientType = "CT_" + cbuId.toString().substring(0, 8);
                CbuCommand.Create createParent = new CbuCommand.Create(
                    cbuId, actor, cbuName, uniqueJurisdiction, null, null, uniqueClientType, null, null, null
                );
                CbuExecutor.execute(conn, createParent);

                CbuCommand.Create createChild = new CbuCommand.Create(
                    childCbuId, actor, "Test Child CBU " + childCbuId, "LU", null, null, "FUND", null, null, null
                );
                CbuExecutor.execute(conn, createChild);

                // 2. Insert entities
                String entityName1 = "John Doe " + entityId1.toString().substring(0, 8);
                String entityName2 = "Acme Private Company " + entityId2.toString().substring(0, 8);
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".entities (entity_id, entity_type_id, name, name_norm, row_version) VALUES (?, ?, ?, ?, 1)")) {
                    ps.setObject(1, entityId1);
                    ps.setObject(2, UUID.fromString("6f7b4e87-363e-4ee3-a717-d4b456d4eec4")); // PROPER_PERSON_NATURAL
                    ps.setString(3, entityName1);
                    ps.setString(4, entityName1.toLowerCase());
                    ps.executeUpdate();
                    
                    ps.setObject(1, entityId2);
                    ps.setObject(2, UUID.fromString("7803ffb7-935e-4cba-aa70-c9bb4cb43509")); // LIMITED_COMPANY_PRIVATE
                    ps.setString(3, entityName2);
                    ps.setString(4, entityName2.toLowerCase());
                    ps.executeUpdate();
                }

                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".entity_proper_persons (proper_person_id, entity_id, first_name, last_name, nationality, person_state) VALUES (?, ?, ?, ?, ?, 'IDENTIFIED')")) {
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, entityId1);
                    ps.setString(3, "John");
                    ps.setString(4, "Doe " + entityId1.toString().substring(0, 8));
                    ps.setString(5, "US");
                    ps.executeUpdate();
                }

                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".entity_limited_companies (limited_company_id, entity_id, company_name, jurisdiction, ubo_status) VALUES (?, ?, ?, ?, 'PENDING')")) {
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, entityId2);
                    ps.setString(3, entityName2);
                    ps.setString(4, "GB");
                    ps.executeUpdate();
                }

                // 3. Assign roles
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, effective_from, effective_to, version) VALUES (?, ?, ?, ?, ?, ?, 1)")) {
                    // John Doe as UBO
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, cbuId);
                    ps.setObject(3, entityId1);
                    ps.setObject(4, UUID.fromString("cad4bc4a-ac3b-40c9-a322-28d9e7236e8b")); // UBO
                    ps.setObject(5, java.sql.Date.valueOf(LocalDate.now().minusDays(1)));
                    ps.setObject(6, java.sql.Date.valueOf(LocalDate.now().plusDays(10)));
                    ps.executeUpdate();

                    // Acme Private Company as INVESTMENT_MANAGER
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, cbuId);
                    ps.setObject(3, entityId2);
                    ps.setObject(4, UUID.fromString("6359b362-8578-4049-a0f2-4e06609a769a")); // INVESTMENT_MANAGER
                    ps.setObject(5, java.sql.Date.valueOf(LocalDate.now().minusDays(1)));
                    ps.setObject(6, java.sql.Date.valueOf(LocalDate.now().plusDays(10)));
                    ps.executeUpdate();
                }

                // 4. Insert documents
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".document_catalog (doc_id, cbu_id, document_type_id, document_name, status, document_id, extraction_status) VALUES (?, ?, ?, ?, ?, ?, 'PENDING')")) {
                    ps.setObject(1, docId);
                    ps.setObject(2, cbuId);
                    ps.setObject(3, UUID.fromString("f14f4808-9181-4223-bdd9-e75b0d2566fe")); // DSL.CBU.MODEL
                    ps.setString(4, "CBU Model Document");
                    ps.setString(5, "active");
                    ps.setObject(6, UUID.randomUUID());
                    ps.executeUpdate();
                }

                // 5. Insert services (Custody + INCOME_COLLECT)
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".service_delivery_map (delivery_id, cbu_id, product_id, service_id, delivery_status, service_config, requested_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, '{}'::jsonb, NOW(), NOW(), NOW())")) {
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, cbuId);
                    ps.setObject(3, UUID.fromString("15244192-0e29-4cd4-8d3b-ec19488ad814")); // Custody
                    ps.setObject(4, UUID.fromString("631b59f4-7317-432c-a4b2-64c12c643cdd")); // INCOME_COLLECT
                    ps.setString(5, "DELIVERED");
                    ps.executeUpdate();
                }

                // 6. Insert structure links
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_structure_links (link_id, parent_cbu_id, child_cbu_id, relationship_type, relationship_selector, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, NOW(), NOW())")) {
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, cbuId);
                    ps.setObject(3, childCbuId);
                    ps.setString(4, "FEEDER");
                    ps.setString(5, "feeder:us");
                    ps.setString(6, "ACTIVE");
                    ps.executeUpdate();
                }

                // 7. Insert contracts, rate cards, and subscriptions
                String clientLabel = "test_client_" + contractId.toString().substring(0, 8);
                String contractRef = "MSA-TEST-" + contractId.toString().substring(0, 8);
                String rateCardName = "Test Rate Card " + rateCardId.toString().substring(0, 8);
                String productCode = "TEST_PROD_" + cbuId.toString().substring(0, 8);

                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".legal_contracts (contract_id, client_label, contract_reference, status, effective_date, created_at, updated_at) VALUES (?, ?, ?, ?, ?, NOW(), NOW())")) {
                    ps.setObject(1, contractId);
                    ps.setString(2, clientLabel);
                    ps.setString(3, contractRef);
                    ps.setString(4, "ACTIVE");
                    ps.setDate(5, java.sql.Date.valueOf(LocalDate.now()));
                    ps.executeUpdate();
                }

                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".rate_cards (rate_card_id, name, currency, effective_date, created_at, updated_at) VALUES (?, ?, ?, ?, NOW(), NOW())")) {
                    ps.setObject(1, rateCardId);
                    ps.setString(2, rateCardName);
                    ps.setString(3, "USD");
                    ps.setDate(4, java.sql.Date.valueOf(LocalDate.now()));
                    ps.executeUpdate();
                }

                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".contract_products (contract_id, product_code, rate_card_id, effective_date, created_at, updated_at) VALUES (?, ?, ?, ?, NOW(), NOW())")) {
                    ps.setObject(1, contractId);
                    ps.setString(2, productCode);
                    ps.setObject(3, rateCardId);
                    ps.setDate(4, java.sql.Date.valueOf(LocalDate.now()));
                    ps.executeUpdate();
                }

                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_subscriptions (cbu_id, contract_id, product_code, status, subscribed_at, created_at, updated_at) VALUES (?, ?, ?, ?, NOW(), NOW(), NOW())")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, contractId);
                    ps.setString(3, productCode);
                    ps.setString(4, "ACTIVE");
                    ps.executeUpdate();
                }

                // 8. Insert evidence
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_evidence (evidence_id, cbu_id, document_id, attestation_ref, evidence_type, evidence_category, description, attached_at, attached_by, verification_status) VALUES (?, ?, ?, ?, ?, ?, ?, NOW(), ?, 'VERIFIED')")) {
                    ps.setObject(1, evidenceId);
                    ps.setObject(2, cbuId);
                    ps.setObject(3, docId);
                    ps.setString(4, "ATT-123");
                    ps.setString(5, "DOCUMENT");
                    ps.setString(6, "KYC");
                    ps.setString(7, "Verification description");
                    ps.setString(8, "test-user");
                    ps.executeUpdate();
                }

                conn.commit();

                // ------------------ VERIFICATION GATE ------------------

                // A. cbu.read
                CbuDto javaRead = CbuReadProjection.read(conn, cbuId);
                JsonNode rustRead = runRustQueryJson("(cbu.read :cbu-id \"" + cbuId + "\")");
                assertNotNull(javaRead);
                assertEquals(rustRead.get("cbu_id").asText(), javaRead.cbuId().toString());
                assertEquals(rustRead.get("name").asText(), javaRead.name());
                assertEquals(rustRead.get("jurisdiction").asText(), javaRead.jurisdiction());
                assertEquals(rustRead.get("client_type").asText(), javaRead.clientType());
                assertEquals(rustRead.get("status").asText(), javaRead.status());
                assertEquals(rustRead.get("disposition_status").asText(), javaRead.dispositionStatus());

                // B. cbu.list
                List<CbuDto> javaList = CbuReadProjection.list(conn, "DISCOVERED", uniqueClientType, uniqueJurisdiction, 10, 0);
                JsonNode rustList = runRustQueryJson("(cbu.list :status \"DISCOVERED\" :client-type \"" + uniqueClientType + "\" :jurisdiction \"" + uniqueJurisdiction + "\" :limit 10 :offset 0)");
                assertTrue(javaList.size() >= 1);
                CbuDto listCbu = javaList.stream().filter(d -> d.cbuId().equals(cbuId)).findFirst().orElse(null);
                assertNotNull(listCbu);
                JsonNode rustListCbu = null;
                for (JsonNode node : rustList) {
                    if (node.get("cbu_id").asText().equals(cbuId.toString())) {
                        rustListCbu = node;
                        break;
                    }
                }
                assertNotNull(rustListCbu);
                assertEquals(rustListCbu.get("name").asText(), listCbu.name());

                // C. cbu.inspect
                CbuInspectDto javaInspect = CbuReadProjection.inspect(conn, cbuId, LocalDate.now());
                JsonNode rustInspect = runRustQueryJson("(cbu.inspect :cbu-id \"" + cbuId + "\")");
                assertNotNull(javaInspect);
                assertEquals(rustInspect.get("cbu_id").asText(), javaInspect.cbuId().toString());
                assertEquals(rustInspect.get("name").asText(), javaInspect.name());
                assertEquals(rustInspect.get("summary").get("entity_count").asInt(), javaInspect.summary().entityCount());
                assertEquals(rustInspect.get("summary").get("document_count").asInt(), javaInspect.summary().documentCount());
                assertEquals(rustInspect.get("summary").get("service_count").asInt(), javaInspect.summary().serviceCount());

                // Inspect Entities
                assertEquals(rustInspect.get("entities").size(), javaInspect.entities().size());
                for (int i = 0; i < javaInspect.entities().size(); i++) {
                    var javaEnt = javaInspect.entities().get(i);
                    var rustEnt = rustInspect.get("entities").get(i);
                    assertEquals(rustEnt.get("entity_id").asText(), javaEnt.entityId().toString());
                    assertEquals(rustEnt.get("name").asText(), javaEnt.name());
                    assertEquals(rustEnt.get("entity_type").asText(), javaEnt.entityType());
                    assertEquals(rustEnt.get("roles").size(), javaEnt.roles().size());
                    for (int j = 0; j < javaEnt.roles().size(); j++) {
                        assertEquals(rustEnt.get("roles").get(j).asText(), javaEnt.roles().get(j));
                    }
                }

                // Inspect Documents
                assertEquals(rustInspect.get("documents").size(), javaInspect.documents().size());
                for (int i = 0; i < javaInspect.documents().size(); i++) {
                    var javaDoc = javaInspect.documents().get(i);
                    var rustDoc = rustInspect.get("documents").get(i);
                    assertEquals(rustDoc.get("doc_id").asText(), javaDoc.docId().toString());
                    assertEquals(rustDoc.get("name").asText(), javaDoc.name());
                    assertEquals(rustDoc.get("type_code").asText(), javaDoc.typeCode());
                }

                // Inspect Services
                assertEquals(rustInspect.get("services").size(), javaInspect.services().size());
                for (int i = 0; i < javaInspect.services().size(); i++) {
                    var javaSvc = javaInspect.services().get(i);
                    var rustSvc = rustInspect.get("services").get(i);
                    assertEquals(rustSvc.get("delivery_id").asText(), javaSvc.deliveryId().toString());
                    assertEquals(rustSvc.get("product_code").asText(), javaSvc.productCode());
                    assertEquals(rustSvc.get("service").asText(), javaSvc.service());
                }

                // D. cbu.parties
                List<CbuPartyDto> javaParties = CbuReadProjection.parties(conn, cbuId, LocalDate.now());
                JsonNode rustParties = runRustQueryJson("(cbu.parties :cbu-id \"" + cbuId + "\")");
                assertEquals(rustParties.size(), javaParties.size());
                for (int i = 0; i < javaParties.size(); i++) {
                    var javaParty = javaParties.get(i);
                    var rustParty = rustParties.get(i);
                    assertEquals(rustParty.get("entity_id").asText(), javaParty.entityId().toString());
                    assertEquals(rustParty.get("entity_name").asText(), javaParty.entityName());
                    assertEquals(rustParty.get("role_name").asText(), javaParty.roleName());
                }

                // E. cbu.list-subscriptions / products
                List<CbuSubscriptionDto> javaSubs = CbuReadProjection.listSubscriptions(conn, cbuId, null);
                JsonNode rustSubs = runRustQueryJson("(cbu.list-subscriptions :cbu-id \"" + cbuId + "\")");
                assertEquals(rustSubs.size(), javaSubs.size());
                for (int i = 0; i < javaSubs.size(); i++) {
                    var javaSub = javaSubs.get(i);
                    var rustSub = rustSubs.get(i);
                    assertEquals(rustSub.get("contract_id").asText(), javaSub.contractId().toString());
                    assertEquals(rustSub.get("product_code").asText(), javaSub.productCode());
                    assertEquals(rustSub.get("rate_card_name").asText(), javaSub.rateCardName());
                    assertEquals(rustSub.get("rate_card_currency").asText(), javaSub.rateCardCurrency());
                }

                // F. cbu.list-evidence
                List<CbuEvidenceDto> javaEv = CbuReadProjection.listEvidence(conn, cbuId, null, null);
                JsonNode rustEv = runRustQueryJson("(cbu.list-evidence :cbu-id \"" + cbuId + "\")");
                assertEquals(rustEv.size(), javaEv.size());
                for (int i = 0; i < javaEv.size(); i++) {
                    var javaE = javaEv.get(i);
                    var rustE = rustEv.get(i);
                    assertEquals(rustE.get("evidence_id").asText(), javaE.evidenceId().toString());
                    assertEquals(rustE.get("evidence_type").asText(), javaE.evidenceType());
                    assertEquals(rustE.get("verification_status").asText(), javaE.verificationStatus());
                }

                // G. cbu.list-structure-links
                List<CbuStructureLinkDto> javaLinks = CbuReadProjection.listStructureLinks(conn, cbuId, null, null, "parent", null);
                JsonNode rustLinks = runRustQueryJson("(cbu.list-structure-links :parent-cbu-id \"" + cbuId + "\")");
                assertEquals(rustLinks.size(), javaLinks.size());
                for (int i = 0; i < javaLinks.size(); i++) {
                    var javaLink = javaLinks.get(i);
                    var rustLink = rustLinks.get(i);
                    assertEquals(rustLink.get("link_id").asText(), javaLink.linkId().toString());
                    assertEquals(rustLink.get("relationship_type").asText(), javaLink.relationshipType());
                    assertEquals(rustLink.get("status").asText(), javaLink.status());
                }

                // H. cbu.validate-option-coverage
                CbuOptionCoverageDto javaCov = CbuReadProjection.validateOptionCoverage(conn, cbuId, null, "ASSET_PRICING", null);
                JsonNode rustCov = runRustQueryJson("(cbu.validate-option-coverage :cbu-id \"" + cbuId + "\" :service \"ASSET_PRICING\")");
                assertNotNull(javaCov);
                assertEquals(rustCov.get("status").asText(), javaCov.status());
                assertEquals(rustCov.get("gaps").size(), javaCov.gaps().size());
                for (int i = 0; i < javaCov.gaps().size(); i++) {
                    assertEquals(rustCov.get("gaps").get(i).get("option_key").asText(), javaCov.gaps().get(i).optionKey());
                }

            } finally {
                // Cleanup in reverse order of foreign keys
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".service_delivery_map WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_evidence WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".document_catalog WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_subscriptions WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".contract_products WHERE contract_id = ?")) {
                    ps.setObject(1, contractId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".rate_cards WHERE rate_card_id = ?")) {
                    ps.setObject(1, rateCardId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".legal_contracts WHERE contract_id = ?")) {
                    ps.setObject(1, contractId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_structure_links WHERE parent_cbu_id = ? OR child_cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".entity_limited_companies WHERE entity_id = ?")) {
                    ps.setObject(1, entityId2);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".entity_proper_persons WHERE entity_id = ?")) {
                    ps.setObject(1, entityId1);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".entities WHERE entity_id IN (?, ?)")) {
                    ps.setObject(1, entityId1);
                    ps.setObject(2, entityId2);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE cbu_id IN (?, ?)")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, childCbuId);
                    ps.executeUpdate();
                }
                conn.commit();
            }
        }
    }

    private JsonNode runRustQueryJson(String dsl) throws IOException, InterruptedException {
        String dbUrl = "postgresql://adamtc007@localhost:5432/data_designer";
        String[] cmd = {
            "cargo", "run",
            "--manifest-path", "../rust/Cargo.toml",
            "--bin", "dsl_cli",
            "--features", "cli",
            "--",
            "-o", "json",
            "execute",
            "--db-url", dbUrl
        };
        ProcessBuilder pb = new ProcessBuilder(cmd);
        Process p = pb.start();
        
        try (var out = p.getOutputStream()) {
            out.write(dsl.getBytes());
            out.flush();
        }
        
        byte[] stdoutBytes;
        byte[] stderrBytes;
        try (var stdout = p.getInputStream(); var stderr = p.getErrorStream()) {
            stdoutBytes = stdout.readAllBytes();
            stderrBytes = stderr.readAllBytes();
        }
        
        int exitCode = p.waitFor();
        if (exitCode != 0) {
            throw new RuntimeException("Rust dsl_cli failed with exit code " + exitCode + "\nError:\n" + new String(stderrBytes) + "\nOutput:\n" + new String(stdoutBytes));
        }
        
        com.fasterxml.jackson.databind.ObjectMapper mapper = new com.fasterxml.jackson.databind.ObjectMapper();
        JsonNode root = mapper.readTree(stdoutBytes);
        return root.get("results").get(0).get("result");
    }

    @Test
    public void testPhase4VerbsDifferentialConformance() throws Exception {
        UUID parentCbuId = UUID.randomUUID();
        UUID childCbuId = UUID.randomUUID();
        UUID entityId = UUID.randomUUID();
        UUID groupId = UUID.randomUUID();
        UUID mancoEntityId = UUID.randomUUID();
        UUID roleId = UUID.fromString("cad4bc4a-ac3b-40c9-a322-28d9e7236e8b"); // UBO

        String parentName = "Phase4 Parent CBU " + parentCbuId;
        String childName = "Phase4 Child CBU " + childCbuId;
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));

        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // 1. Create parent and child CBUs
                CbuCommand.Create createParent = new CbuCommand.Create(
                    parentCbuId, actor, parentName, "LU", null, null, "FUND", null, null, null
                );
                CbuExecutor.execute(conn, createParent);

                CbuCommand.Create createChild = new CbuCommand.Create(
                    childCbuId, actor, childName, "LU", null, null, "FUND", null, null, null
                );
                CbuExecutor.execute(conn, createChild);

                // 2. Insert entities
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".entities (entity_id, entity_type_id, name, name_norm, row_version) VALUES (?, ?, ?, ?, 1)")) {
                    // John Doe (NAT_PERSON)
                    ps.setObject(1, entityId);
                    ps.setObject(2, UUID.fromString("6f7b4e87-363e-4ee3-a717-d4b456d4eec4")); 
                    ps.setString(3, "John Doe " + entityId.toString().substring(0, 8));
                    ps.setString(4, ("John Doe " + entityId.toString().substring(0, 8)).toLowerCase());
                    ps.executeUpdate();

                    // ManCo Entity (LIMITED_COMPANY_PRIVATE)
                    ps.setObject(1, mancoEntityId);
                    ps.setObject(2, UUID.fromString("7803ffb7-935e-4cba-aa70-c9bb4cb43509"));
                    ps.setString(3, "ManCo Entity " + mancoEntityId.toString().substring(0, 8));
                    ps.setString(4, ("ManCo Entity " + mancoEntityId.toString().substring(0, 8)).toLowerCase());
                    ps.executeUpdate();
                }

                // 3. Insert group
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_groups (group_id, manco_entity_id, group_name) VALUES (?, ?, ?)")) {
                    ps.setObject(1, groupId);
                    ps.setObject(2, mancoEntityId);
                    ps.setString(3, "Test Group " + groupId);
                    ps.executeUpdate();
                }

                conn.commit();

                // ------------------ A. cbu.link-structure ------------------
                // Execute in Rust
                String rustLinkDsl = String.format(
                    "(cbu.link-structure :parent-cbu-id \"%s\" :child-cbu-id \"%s\" :relationship-type \"FEEDER\" :relationship-selector \"feeder:us\" :capital-flow \"UPSTREAM\")",
                    parentCbuId, childCbuId
                );
                executeRustDsl(rustLinkDsl);

                // Recover Rust state
                UUID rustLinkId = null;
                String rustSelector = null;
                String rustFlow = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT link_id, relationship_selector, capital_flow FROM \"ob-poc\".cbu_structure_links WHERE parent_cbu_id = ? AND child_cbu_id = ?")) {
                    ps.setObject(1, parentCbuId);
                    ps.setObject(2, childCbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) {
                            rustLinkId = (UUID) rs.getObject("link_id");
                            rustSelector = rs.getString("relationship_selector");
                            rustFlow = rs.getString("capital_flow");
                        }
                    }
                }
                assertNotNull(rustLinkId);
                assertEquals("feeder:us", rustSelector);
                assertEquals("UPSTREAM", rustFlow);

                // Reset database link
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_structure_links WHERE parent_cbu_id = ? AND child_cbu_id = ?")) {
                    ps.setObject(1, parentCbuId);
                    ps.setObject(2, childCbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Run Java LinkStructure
                CbuCommand.LinkStructure javaLink = new CbuCommand.LinkStructure(
                    parentCbuId, actor, childCbuId, "FEEDER", "feeder:us", "UPSTREAM", null, null
                );
                CbuExecutionResult linkRes = CbuExecutor.execute(conn, javaLink);
                assertTrue(linkRes instanceof CbuExecutionResult.Success);

                // Verify Java state matches Rust
                UUID javaLinkId = null;
                String javaSelector = null;
                String javaFlow = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT link_id, relationship_selector, capital_flow FROM \"ob-poc\".cbu_structure_links WHERE parent_cbu_id = ? AND child_cbu_id = ?")) {
                    ps.setObject(1, parentCbuId);
                    ps.setObject(2, childCbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) {
                            javaLinkId = (UUID) rs.getObject("link_id");
                            javaSelector = rs.getString("relationship_selector");
                            javaFlow = rs.getString("capital_flow");
                        }
                    }
                }
                assertNotNull(javaLinkId);
                assertEquals(rustSelector, javaSelector);
                assertEquals(rustFlow, javaFlow);

                // ------------------ B. cbu.unlink-structure ------------------
                // Run Rust Unlink
                String rustUnlinkDsl = String.format("(cbu.unlink-structure :link-id \"%s\" :reason \"Rust unlink\" :hard-delete false)", javaLinkId);
                executeRustDsl(rustUnlinkDsl);

                String rustLinkStatus = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT status FROM \"ob-poc\".cbu_structure_links WHERE link_id = ?")) {
                    ps.setObject(1, javaLinkId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) {
                            rustLinkStatus = rs.getString("status");
                        }
                    }
                }
                assertEquals("TERMINATED", rustLinkStatus);

                // Reset back to ACTIVE
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_structure_links SET status = 'ACTIVE', terminated_at = NULL, terminated_reason = NULL WHERE link_id = ?")) {
                    ps.setObject(1, javaLinkId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Run Java Unlink
                CbuCommand.UnlinkStructure javaUnlink = new CbuCommand.UnlinkStructure(javaLinkId, actor, "Rust unlink", false);
                CbuExecutionResult unlinkRes = CbuExecutor.execute(conn, javaUnlink);
                assertTrue(unlinkRes instanceof CbuExecutionResult.Success);

                String javaLinkStatus = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT status FROM \"ob-poc\".cbu_structure_links WHERE link_id = ?")) {
                    ps.setObject(1, javaLinkId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) {
                            javaLinkStatus = rs.getString("status");
                        }
                    }
                }
                assertEquals("TERMINATED", javaLinkStatus);

                // ------------------ C. cbu-role.terminate ------------------
                // Seed a role
                UUID userRoleId = UUID.randomUUID();
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id) VALUES (?, ?, ?, ?)")) {
                    ps.setObject(1, userRoleId);
                    ps.setObject(2, parentCbuId);
                    ps.setObject(3, entityId);
                    ps.setObject(4, roleId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Run Rust terminate
                String rustTerminateDsl = String.format("(cbu-role.terminate :cbu-id \"%s\" :hard-delete false)", parentCbuId);
                executeRustDsl(rustTerminateDsl);

                Date rustRoleTo = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT effective_to FROM \"ob-poc\".cbu_entity_roles WHERE cbu_entity_role_id = ?")) {
                    ps.setObject(1, userRoleId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) {
                            rustRoleTo = rs.getDate("effective_to");
                        }
                    }
                }
                assertNotNull(rustRoleTo);

                // Reset role
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_entity_roles SET effective_to = NULL WHERE cbu_entity_role_id = ?")) {
                    ps.setObject(1, userRoleId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Run Java terminate
                CbuCommand.TerminateRole javaTerminate = new CbuCommand.TerminateRole(parentCbuId, actor, false);
                CbuExecutionResult termRes = CbuExecutor.execute(conn, javaTerminate);
                assertTrue(termRes instanceof CbuExecutionResult.Success);

                Date javaRoleTo = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT effective_to FROM \"ob-poc\".cbu_entity_roles WHERE cbu_entity_role_id = ?")) {
                    ps.setObject(1, userRoleId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) {
                            javaRoleTo = rs.getDate("effective_to");
                        }
                    }
                }
                assertNotNull(javaRoleTo);
                assertEquals(rustRoleTo.toString(), javaRoleTo.toString());

                // Assert event emitted
                List<Object> termEvents = ((CbuExecutionResult.Success) termRes).events();
                boolean termEventFound = false;
                for (Object evt : termEvents) {
                    if (evt instanceof Effect.EmitPendingStateAdvance) {
                        Effect.EmitPendingStateAdvance eps = (Effect.EmitPendingStateAdvance) evt;
                        if ("cbu-role:terminated".equals(eps.stateAdvanceKey())) {
                            termEventFound = true;
                            assertEquals("cbu/role-graph", eps.advancementType());
                            assertEquals("cbu-role.terminate", eps.description());
                        }
                    }
                }
                assertTrue(termEventFound);

                // ------------------ D. cbu-group.remove-member ------------------
                // Seed membership
                UUID membershipId = UUID.randomUUID();
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_group_members (membership_id, group_id, cbu_id) VALUES (?, ?, ?)")) {
                    ps.setObject(1, membershipId);
                    ps.setObject(2, groupId);
                    ps.setObject(3, parentCbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Run Rust remove-member
                String rustRemoveDsl = String.format("(cbu-group.remove-member :cbu-id \"%s\" :hard-delete false)", parentCbuId);
                executeRustDsl(rustRemoveDsl);

                Date rustMemberTo = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT effective_to FROM \"ob-poc\".cbu_group_members WHERE membership_id = ?")) {
                    ps.setObject(1, membershipId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) {
                            rustMemberTo = rs.getDate("effective_to");
                        }
                    }
                }
                assertNotNull(rustMemberTo);

                // Reset member
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_group_members SET effective_to = NULL WHERE membership_id = ?")) {
                    ps.setObject(1, membershipId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Run Java remove-member
                CbuCommand.RemoveMember javaRemove = new CbuCommand.RemoveMember(parentCbuId, actor, false);
                CbuExecutionResult removeRes = CbuExecutor.execute(conn, javaRemove);
                assertTrue(removeRes instanceof CbuExecutionResult.Success);

                Date javaMemberTo = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT effective_to FROM \"ob-poc\".cbu_group_members WHERE membership_id = ?")) {
                    ps.setObject(1, membershipId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) {
                            javaMemberTo = rs.getDate("effective_to");
                        }
                    }
                }
                assertNotNull(javaMemberTo);
                assertEquals(rustMemberTo.toString(), javaMemberTo.toString());

                // Assert event emitted
                List<Object> removeEvents = ((CbuExecutionResult.Success) removeRes).events();
                boolean removeEventFound = false;
                for (Object evt : removeEvents) {
                    if (evt instanceof Effect.EmitPendingStateAdvance) {
                        Effect.EmitPendingStateAdvance eps = (Effect.EmitPendingStateAdvance) evt;
                        if ("cbu-group-member:removed".equals(eps.stateAdvanceKey())) {
                            removeEventFound = true;
                            assertEquals("cbu/group-membership", eps.advancementType());
                            assertEquals("cbu-group.remove-member", eps.description());
                        }
                    }
                }
                assertTrue(removeEventFound);

            } finally {
                // Cleanup database
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_structure_links WHERE parent_cbu_id = ? OR child_cbu_id = ?")) {
                    ps.setObject(1, parentCbuId);
                    ps.setObject(2, parentCbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?")) {
                    ps.setObject(1, parentCbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_group_members WHERE cbu_id = ?")) {
                    ps.setObject(1, parentCbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_groups WHERE group_id = ?")) {
                    ps.setObject(1, groupId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".entities WHERE entity_id IN (?, ?)")) {
                    ps.setObject(1, entityId);
                    ps.setObject(2, mancoEntityId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE cbu_id IN (?, ?)")) {
                    ps.setObject(1, parentCbuId);
                    ps.setObject(2, childCbuId);
                    ps.executeUpdate();
                }
                conn.commit();
            }
        }
    }
}
