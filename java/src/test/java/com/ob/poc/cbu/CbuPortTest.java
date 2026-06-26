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
                executeRustDsl("(cbu.soft-delete :cbu-id \"" + cbuId + "\" :reason \"prepare\")");
                executeRustDsl("(cbu.hard-delete :cbu-id \"" + cbuId + "\")");
                CbuStatus rustHardDeleted = CbuRepository.recover(conn, cbuId);
                assertEquals("hard_deleted", rustHardDeleted.rawDispStatus());

                // Reset Java side disposition_status back to soft_deleted
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET disposition_status = 'soft_deleted' WHERE cbu_id = ?")) {
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

    @Test
    public void testStaleStatusTransitionGuard() throws Exception {
        UUID cbuId = UUID.randomUUID();
        String cbuName = "Stale Guard CBU " + cbuId;
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));

        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // 1. Create a CBU and set status to VALIDATION_PENDING
                CbuCommand.Create createCmd = new CbuCommand.Create(
                    cbuId, actor, cbuName, "LU", null, null, "FUND", null, null, null
                );
                CbuExecutor.execute(conn, createCmd);

                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'VALIDATION_PENDING' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // 2. Wrap Connection in a dynamic proxy to simulate concurrent modification right before the update statement
                Connection proxyConn = (Connection) java.lang.reflect.Proxy.newProxyInstance(
                    Connection.class.getClassLoader(),
                    new Class<?>[] { Connection.class },
                    (proxy, method, methodArgs) -> {
                        if ("prepareStatement".equals(method.getName()) && methodArgs.length > 0 && methodArgs[0] instanceof String sql) {
                            if (sql.contains("UPDATE") && (sql.contains("status") || sql.contains("validation"))) {
                                // This is the status update query! Modify the state concurrently on a separate connection
                                try (Connection other = getDbConnection()) {
                                    try (PreparedStatement ps = other.prepareStatement("UPDATE \"ob-poc\".cbus SET status = 'VALIDATED' WHERE cbu_id = ?")) {
                                        ps.setObject(1, cbuId);
                                        ps.executeUpdate();
                                    }
                                }
                            }
                        }
                        try {
                            return method.invoke(conn, methodArgs);
                        } catch (java.lang.reflect.InvocationTargetException e) {
                            throw e.getTargetException();
                        }
                    }
                );

                // 3. Execute Java Confirm via proxy connection. It should fail because the status was changed to VALIDATED concurrently.
                CbuCommand.Confirm confirmCmd = new CbuCommand.Confirm(cbuId, actor);
                CbuExecutionResult result = CbuExecutor.execute(proxyConn, confirmCmd);

                assertTrue(result instanceof CbuExecutionResult.Failure);
                CbuExecutionResult.Failure failure = (CbuExecutionResult.Failure) result;
                assertTrue(failure.reason().toLowerCase().contains("concurrent modification") || failure.reason().toLowerCase().contains("precondition"));

            } finally {
                // Cleanup
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();
            }
        }
    }

    @Test
    public void testIdempotentCreateIdDifferentialConformance() throws Exception {
        UUID id2 = UUID.randomUUID();
        String cbuName = "Idempotent Create ID CBU " + UUID.randomUUID();
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));

        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // First call: create via Rust (so we establish the first ID/Rust ID)
                String rustDsl = String.format(
                    "(cbu.create :name \"%s\" :jurisdiction \"LU\" :client-type \"FUND\" :nature-purpose \"test\" :description \"desc\")",
                    cbuName
                );
                executeRustDsl(rustDsl);

                // Recover actual ID created by Rust
                CbuStatus statusAfterRust = CbuRepository.recoverByNameAndJurisdiction(conn, cbuName, "LU");
                assertNotNull(statusAfterRust);
                UUID rustCbuId = statusAfterRust.id();

                // Second call: call Java create twice, but with id2 as requested ID on the second call
                CbuCommand.Create createJava2 = new CbuCommand.Create(
                    id2, actor, cbuName, "LU", null, null, "FUND", "test", "desc", null
                );

                CbuExecutionResult res = CbuExecutor.execute(conn, createJava2);
                assertTrue(res instanceof CbuExecutionResult.Success);
                CbuExecutionResult.Success success = (CbuExecutionResult.Success) res;

                // The returned ID must match the first ID (rustCbuId), not the requested ID (id2)
                assertEquals(rustCbuId, success.cbuId());
                assertFalse(success.created()); // should be false since it was already created

            } finally {
                // Cleanup
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE name = ?")) {
                    ps.setString(1, cbuName);
                    ps.executeUpdate();
                }
                conn.commit();
            }
        }
    }

    @Test
    public void testSettersDifferentialConformance() throws Exception {
        UUID cbuId = UUID.randomUUID();
        String cbuName = "Setter Test CBU " + cbuId;
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer", "compliance_admin"));

        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // Initialize CBU in Java (Discovered/Active)
                CbuCommand.Create createCmd = new CbuCommand.Create(
                    cbuId, actor, cbuName, "LU", null, null, "FUND", "nature", "desc", null
                );
                CbuExecutor.execute(conn, createCmd);
                conn.commit();

                // 1. Rename
                String newName = "Renamed " + cbuName;
                executeRustDsl(String.format("(cbu.rename :cbu-id \"%s\" :name \"%s\")", cbuId, newName));
                CbuStatus rustRenamed = CbuRepository.recover(conn, cbuId);
                assertEquals(newName, rustRenamed.name());

                // Reset
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET name = ? WHERE cbu_id = ?")) {
                    ps.setString(1, cbuName);
                    ps.setObject(2, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Rename
                CbuExecutionResult renameRes = CbuExecutor.execute(conn, new CbuCommand.Rename(cbuId, actor, newName));
                assertTrue(renameRes instanceof CbuExecutionResult.Success);
                CbuStatus javaRenamed = CbuRepository.recover(conn, cbuId);
                assertEquals(rustRenamed.name(), javaRenamed.name());

                // 2. SetJurisdiction
                executeRustDsl(String.format("(cbu.set-jurisdiction :cbu-id \"%s\" :jurisdiction \"US\")", cbuId));
                CbuStatus rustJurisdiction = CbuRepository.recover(conn, cbuId);
                assertEquals("US", rustJurisdiction.jurisdiction());

                // Reset
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET jurisdiction = 'LU' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java SetJurisdiction
                CbuExecutionResult jurRes = CbuExecutor.execute(conn, new CbuCommand.SetJurisdiction(cbuId, actor, "US"));
                assertTrue(jurRes instanceof CbuExecutionResult.Success);
                CbuStatus javaJurisdiction = CbuRepository.recover(conn, cbuId);
                assertEquals(rustJurisdiction.jurisdiction(), javaJurisdiction.jurisdiction());

                // 3. SetClientType
                executeRustDsl(String.format("(cbu.set-client-type :cbu-id \"%s\" :client-type \"CORPORATE\")", cbuId));
                CbuStatus rustClientType = CbuRepository.recover(conn, cbuId);
                assertEquals("CORPORATE", rustClientType.clientType());

                // Reset
                try (var ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET client_type = 'FUND' WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java SetClientType
                CbuExecutionResult ctRes = CbuExecutor.execute(conn, new CbuCommand.SetClientType(cbuId, actor, "CORPORATE"));
                assertTrue(ctRes instanceof CbuExecutionResult.Success);
                CbuStatus javaClientType = CbuRepository.recover(conn, cbuId);
                assertEquals(rustClientType.clientType(), javaClientType.clientType());

            } finally {
                // Cleanup
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();
            }
        }
    }

    @Test
    public void testPhase5WriteVerbsDifferentialConformance() throws Exception {
        UUID cbuId = UUID.randomUUID();
        UUID childCbuId = UUID.randomUUID();
        UUID entityId1 = UUID.randomUUID();
        UUID entityId2 = UUID.randomUUID();
        UUID groupId = UUID.randomUUID();
        UUID mancoEntityId = UUID.randomUUID();
        UUID caEventId = UUID.randomUUID();
        UUID docId = UUID.randomUUID();
        UUID evidenceId = UUID.randomUUID();
        UUID optServiceId = UUID.randomUUID();
        UUID optVersionId = UUID.randomUUID();
        UUID optDefId = UUID.randomUUID();

        String cbuName = "Phase5 Test CBU " + cbuId;
        String childName = "Phase5 Child CBU " + childCbuId;
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer", "compliance_admin"));

        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // Initialize CBU in Java
                CbuCommand.Create createParent = new CbuCommand.Create(
                    cbuId, actor, cbuName, "LU", null, null, "FUND", "nature", "desc", null
                );
                CbuExecutor.execute(conn, createParent);

                CbuCommand.Create createChild = new CbuCommand.Create(
                    childCbuId, actor, childName, "LU", null, null, "FUND", "nature", "desc", null
                );
                CbuExecutor.execute(conn, createChild);

                // Insert entities
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".entities (entity_id, entity_type_id, name, name_norm, row_version) VALUES (?, ?, ?, ?, 1)")) {
                    ps.setObject(1, entityId1);
                    ps.setObject(2, UUID.fromString("6f7b4e87-363e-4ee3-a717-d4b456d4eec4")); 
                    ps.setString(3, "John Person " + entityId1.toString().substring(0, 8));
                    ps.setString(4, ("John Person " + entityId1.toString().substring(0, 8)).toLowerCase());
                    ps.executeUpdate();

                    ps.setObject(1, entityId2);
                    ps.setObject(2, UUID.fromString("7803ffb7-935e-4cba-aa70-c9bb4cb43509"));
                    ps.setString(3, "Acme Entity " + entityId2.toString().substring(0, 8));
                    ps.setString(4, ("Acme Entity " + entityId2.toString().substring(0, 8)).toLowerCase());
                    ps.executeUpdate();

                    ps.setObject(1, mancoEntityId);
                    ps.setObject(2, UUID.fromString("7803ffb7-935e-4cba-aa70-c9bb4cb43509"));
                    ps.setString(3, "ManCo Entity " + mancoEntityId.toString().substring(0, 8));
                    ps.setString(4, ("ManCo Entity " + mancoEntityId.toString().substring(0, 8)).toLowerCase());
                    ps.executeUpdate();
                }

                // Insert group
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_groups (group_id, manco_entity_id, group_name) VALUES (?, ?, ?)")) {
                    ps.setObject(1, groupId);
                    ps.setObject(2, mancoEntityId);
                    ps.setString(3, "Test Group " + groupId);
                    ps.executeUpdate();
                }

                // Insert client group
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".client_group (id, canonical_name) VALUES (?, ?)")) {
                    ps.setObject(1, groupId);
                    ps.setString(2, "Test Group " + groupId);
                    ps.executeUpdate();
                }

                // Insert document
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".document_catalog (doc_id, cbu_id, document_type_id, document_name, status, document_id) VALUES (?, ?, ?, ?, 'active', ?)")) {
                    ps.setObject(1, docId);
                    ps.setObject(2, cbuId);
                    ps.setObject(3, UUID.fromString("f14f4808-9181-4223-bdd9-e75b0d2566fe")); // CBU.MODEL
                    ps.setString(4, "Test Doc");
                    ps.setObject(5, UUID.randomUUID());
                    ps.executeUpdate();
                }

                // Insert Proposed CA
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_corporate_action_events (event_id, cbu_id, event_type, ca_status) VALUES (?, ?, 'rename', 'proposed')")) {
                    ps.setObject(1, caEventId);
                    ps.setObject(2, cbuId);
                    ps.executeUpdate();
                }

                // Insert client group entity
                UUID cgeId = UUID.randomUUID();
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".client_group_entity (id, group_id, entity_id, membership_type) VALUES (?, ?, ?, 'confirmed')")) {
                    ps.setObject(1, cgeId);
                    ps.setObject(2, groupId);
                    ps.setObject(3, entityId1);
                    ps.executeUpdate();
                }

                // Insert option service framework components
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".services (service_id, service_code, name) VALUES (?, ?, ?)")) {
                    ps.setObject(1, optServiceId);
                    ps.setString(2, "OPT_SERVICE_" + optServiceId.toString().substring(0,8));
                    ps.setString(3, "Options test service");
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".service_versions (id, service_id, version, lifecycle_status) VALUES (?, ?, 'v1', 'published')")) {
                    ps.setObject(1, optVersionId);
                    ps.setObject(2, optServiceId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".service_option_defs (service_option_def_id, service_id, service_version_id, option_key, option_kind, is_required, lifecycle_status, default_source_kind) VALUES (?, ?, ?, ?, 'string', true, 'active', 'manual')")) {
                    ps.setObject(1, optDefId);
                    ps.setObject(2, optServiceId);
                    ps.setObject(3, optVersionId);
                    ps.setString(4, "opt_key");
                    ps.executeUpdate();
                }

                conn.commit();

                // ------------------ 1. cbu-ca.submit-for-review ------------------
                executeRustDsl("(cbu-ca.submit-for-review :event-id \"" + caEventId + "\")");
                String rustCaStatus1 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals("under_review", rustCaStatus1);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = 'proposed' WHERE event_id = ?")) {
                    ps.setObject(1, caEventId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java SubmitForReview
                CbuExecutor.execute(conn, new CbuCommand.SubmitForReview(caEventId, actor));
                String javaCaStatus1 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals(rustCaStatus1, javaCaStatus1);

                // ------------------ 2. cbu-ca.reject ------------------
                executeRustDsl("(cbu-ca.submit-for-review :event-id \"" + caEventId + "\")");
                executeRustDsl("(cbu-ca.reject :event-id \"" + caEventId + "\" :reason \"No way\")");
                String rustCaStatus2 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals("rejected", rustCaStatus2);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = 'under_review', rejected_reason = null WHERE event_id = ?")) {
                    ps.setObject(1, caEventId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Reject
                CbuExecutor.execute(conn, new CbuCommand.RejectCa(caEventId, actor, "No way"));
                String javaCaStatus2 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals(rustCaStatus2, javaCaStatus2);

                // ------------------ 3. cbu-ca.withdraw ------------------
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = 'proposed' WHERE event_id = ?")) {
                    ps.setObject(1, caEventId);
                    ps.executeUpdate();
                }
                conn.commit();

                executeRustDsl("(cbu-ca.withdraw :event-id \"" + caEventId + "\")");
                String rustCaStatus3 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals("withdrawn", rustCaStatus3);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = 'proposed' WHERE event_id = ?")) {
                    ps.setObject(1, caEventId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Withdraw
                CbuExecutor.execute(conn, new CbuCommand.WithdrawCa(caEventId, actor));
                String javaCaStatus3 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals(rustCaStatus3, javaCaStatus3);

                // ------------------ 4. cbu-ca.approve ------------------
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = 'under_review' WHERE event_id = ?")) {
                    ps.setObject(1, caEventId);
                    ps.executeUpdate();
                }
                conn.commit();

                executeRustDsl("(cbu-ca.approve :event-id \"" + caEventId + "\")");
                String rustCaStatus4 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals("approved", rustCaStatus4);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = 'under_review' WHERE event_id = ?")) {
                    ps.setObject(1, caEventId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Approve
                CbuExecutor.execute(conn, new CbuCommand.ApproveCa(caEventId, actor));
                String javaCaStatus4 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals(rustCaStatus4, javaCaStatus4);

                // ------------------ 5. cbu-ca.mark-implemented ------------------
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = 'effective' WHERE event_id = ?")) {
                    ps.setObject(1, caEventId);
                    ps.executeUpdate();
                }
                conn.commit();

                executeRustDsl("(cbu-ca.mark-implemented :event-id \"" + caEventId + "\")");
                String rustCaStatus5 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals("implemented", rustCaStatus5);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = 'effective' WHERE event_id = ?")) {
                    ps.setObject(1, caEventId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java MarkImplemented
                CbuExecutor.execute(conn, new CbuCommand.MarkImplementedCa(caEventId, actor));
                String javaCaStatus5 = CbuRepository.recoverCaStatus(conn, caEventId);
                assertEquals(rustCaStatus5, javaCaStatus5);

                // ------------------ 6. cbu.assign-role (OWNERSHIP) ------------------
                executeRustDsl(String.format(
                    "(cbu.assign-role :cbu-id \"%s\" :role-type \"ownership\" :role \"SHAREHOLDER\" :owner-entity-id \"%s\" :owned-entity-id \"%s\" :percentage \"45.5\" :effective-from \"2026-01-01\" :ownership-type \"beneficial\")",
                    cbuId, entityId1, entityId2
                ));
                // Fetch Rust roles count
                long rustRoleCount = 0;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustRoleCount = rs.getLong(1);
                    }
                }
                assertEquals(1, rustRoleCount);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".entity_relationships WHERE from_entity_id = ? AND to_entity_id = ?")) {
                    ps.setObject(1, entityId1);
                    ps.setObject(2, entityId2);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java AssignRole
                CbuCommand.AssignRole assignOwnership = new CbuCommand.AssignRole(
                    cbuId, actor, "ownership", "SHAREHOLDER", entityId1, entityId2,
                    null, null, null, null, null, null, null, null, null, null,
                    "45.5", "beneficial", "2026-01-01", null, null, null, null, null,
                    null, null, null, null, null, null, null, null
                );
                CbuExecutor.execute(conn, assignOwnership);
                
                long javaRoleCount = 0;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaRoleCount = rs.getLong(1);
                    }
                }
                assertEquals(rustRoleCount, javaRoleCount);

                // ------------------ 7. cbu.remove-role ------------------
                // Rust remove role
                executeRustDsl(String.format(
                    "(cbu.remove-role :cbu-id \"%s\" :entity-id \"%s\" :role \"SHAREHOLDER\")",
                    cbuId, entityId1
                ));
                long rustRoleCountAfterRemove = 0;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustRoleCountAfterRemove = rs.getLong(1);
                    }
                }
                assertEquals(0, rustRoleCountAfterRemove);

                // Reset (Re-assign via Java)
                CbuExecutor.execute(conn, assignOwnership);
                conn.commit();

                // Java RemoveRole
                CbuCommand.RemoveRole removeCmd = new CbuCommand.RemoveRole(cbuId, actor, entityId1, "SHAREHOLDER");
                CbuExecutor.execute(conn, removeCmd);

                long javaRoleCountAfterRemove = 0;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaRoleCountAfterRemove = rs.getLong(1);
                    }
                }
                assertEquals(rustRoleCountAfterRemove, javaRoleCountAfterRemove);

                // ------------------ 8. cbu.attach-evidence ------------------
                executeRustDsl(String.format(
                    "(cbu.attach-evidence :cbu-id \"%s\" :document-id \"%s\" :evidence-id \"%s\" :attestation-ref \"ATT-999\" :evidence-type \"DOCUMENT\" :evidence-category \"KYC\" :description \"KYC proof\")",
                    cbuId, docId, evidenceId
                ));
                UUID rustEvidenceId = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT evidence_id FROM \"ob-poc\".cbu_evidence WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustEvidenceId = (UUID) rs.getObject(1);
                    }
                }
                assertNotNull(rustEvidenceId);

                long rustEvidenceCount = 0;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_evidence WHERE evidence_id = ?")) {
                    ps.setObject(1, rustEvidenceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustEvidenceCount = rs.getLong(1);
                    }
                }
                assertEquals(1, rustEvidenceCount);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_evidence WHERE evidence_id = ?")) {
                    ps.setObject(1, rustEvidenceId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Attach
                CbuCommand.AttachEvidence attachCmd = new CbuCommand.AttachEvidence(
                    cbuId, actor, docId, "ATT-999", "DOCUMENT", "KYC", "KYC proof", "test-user"
                );
                CbuExecutionResult attachRes = CbuExecutor.execute(conn, attachCmd);
                assertTrue(attachRes instanceof CbuExecutionResult.Success);
                UUID javaEvidenceId = null;
                for (Object evt : ((CbuExecutionResult.Success) attachRes).events()) {
                    if (evt instanceof CbuRepository.AttachEvidenceResult aer) {
                        javaEvidenceId = aer.evidenceId();
                    }
                }
                assertNotNull(javaEvidenceId);

                long javaEvidenceCount = 0;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_evidence WHERE evidence_id = ?")) {
                    ps.setObject(1, javaEvidenceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaEvidenceCount = rs.getLong(1);
                    }
                }
                assertEquals(rustEvidenceCount, javaEvidenceCount);

                // ------------------ 9. cbu.verify-evidence ------------------
                executeRustDsl(String.format(
                    "(cbu.attach-evidence :cbu-id \"%s\" :document-id \"%s\" :evidence-id \"%s\" :attestation-ref \"ATT-999\" :evidence-type \"DOCUMENT\" :evidence-category \"KYC\" :description \"KYC proof\")",
                    cbuId, docId, evidenceId
                ));
                UUID rustEvidenceIdForVerify = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT evidence_id FROM \"ob-poc\".cbu_evidence WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustEvidenceIdForVerify = (UUID) rs.getObject(1);
                    }
                }
                assertNotNull(rustEvidenceIdForVerify);

                executeRustDsl(String.format(
                    "(cbu.verify-evidence :cbu-id \"%s\" :evidence-id \"%s\" :verification-status \"VERIFIED\" :verified-by \"test-user\" :verification-notes \"All good\")",
                    cbuId, rustEvidenceIdForVerify
                ));
                String rustVerificationStatus = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT verification_status FROM \"ob-poc\".cbu_evidence WHERE evidence_id = ?")) {
                    ps.setObject(1, rustEvidenceIdForVerify);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustVerificationStatus = rs.getString(1);
                    }
                }
                assertEquals("VERIFIED", rustVerificationStatus);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_evidence WHERE evidence_id = ?")) {
                    ps.setObject(1, rustEvidenceIdForVerify);
                    ps.executeUpdate();
                }
                // Attach via Java to verify
                CbuExecutionResult attachRes2 = CbuExecutor.execute(conn, attachCmd);
                assertTrue(attachRes2 instanceof CbuExecutionResult.Success);
                UUID javaEvidenceId2 = null;
                for (Object evt : ((CbuExecutionResult.Success) attachRes2).events()) {
                    if (evt instanceof CbuRepository.AttachEvidenceResult aer) {
                        javaEvidenceId2 = aer.evidenceId();
                    }
                }
                assertNotNull(javaEvidenceId2);
                conn.commit();

                // Java Verify
                CbuCommand.VerifyEvidence verifyCmd = new CbuCommand.VerifyEvidence(
                    javaEvidenceId2, actor, "VERIFIED", "test-user", "All good"
                );
                CbuExecutor.execute(conn, verifyCmd);
                String javaVerificationStatus = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT verification_status FROM \"ob-poc\".cbu_evidence WHERE evidence_id = ?")) {
                    ps.setObject(1, javaEvidenceId2);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaVerificationStatus = rs.getString(1);
                    }
                }
                assertEquals(rustVerificationStatus, javaVerificationStatus);

                // ------------------ 10. cbu.ensure ------------------
                UUID ensureId = UUID.randomUUID();
                String ensureName = "Ensure Test CBU " + ensureId;
                executeRustDsl(String.format(
                    "(cbu.ensure :cbu-id \"%s\" :name \"%s\" :jurisdiction \"LU\" :client-type \"FUND\" :nature-purpose \"ensured nature\")",
                    ensureId, ensureName
                ));
                CbuStatus rustEnsured = CbuRepository.recoverByNameAndJurisdiction(conn, ensureName, "LU");
                assertNotNull(rustEnsured);
                assertEquals("FUND", rustEnsured.clientType());
                UUID rustEnsureCbuId = rustEnsured.id();

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE cbu_id = ?")) {
                    ps.setObject(1, rustEnsureCbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Ensure
                CbuCommand.Ensure ensureCmd = new CbuCommand.Ensure(
                    ensureId, actor, ensureName, "LU", "FUND", "ensured nature", null
                );
                CbuExecutor.execute(conn, ensureCmd);
                CbuStatus javaEnsured = CbuRepository.recover(conn, ensureId);
                assertNotNull(javaEnsured);
                assertEquals("FUND", javaEnsured.clientType());

                // Cleanup ensure CBU
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE cbu_id = ?")) {
                    ps.setObject(1, ensureId);
                    ps.executeUpdate();
                }
                conn.commit();

                // ------------------ 11. cbu.create-from-client-group ------------------
                JsonNode rustCg = runRustQueryJson(String.format(
                    "(cbu.create-from-client-group :group-id \"%s\" :default-jurisdiction \"LU\" :limit 10)",
                    groupId
                ));
                CbuCommand.CreateFromClientGroup cgCmd = new CbuCommand.CreateFromClientGroup(
                    groupId, actor, null, null, null, "LU", null, 10, false
                );
                CbuExecutionResult cgRes = CbuExecutor.execute(conn, cgCmd);
                assertTrue(cgRes instanceof CbuExecutionResult.Success);
                List<Object> cgEvents = ((CbuExecutionResult.Success) cgRes).events();
                assertEquals(1, cgEvents.size());
                CbuRepository.CreateFromClientGroupResult cgResult = (CbuRepository.CreateFromClientGroupResult) cgEvents.get(0);
                assertEquals(rustCg.get("entities_found").asInt(), cgResult.entitiesFound());
                assertEquals(rustCg.get("dsl_batch").get(0).asText(), cgResult.dslBatch().get(0));

                // ------------------ 12. cbu.bind-service-options ------------------
                executeRustDsl(String.format(
                    "(cbu.bind-service-options :cbu-id \"%s\" :product-id \"15244192-0e29-4cd4-8d3b-ec19488ad814\" :service-id \"%s\" :options {:opt_key \"val1\"})",
                    cbuId, optServiceId
                ));
                long rustBindingCount = 0;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustBindingCount = rs.getLong(1);
                    }
                }
                assertEquals(1, rustBindingCount);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Bind
                CbuCommand.BindServiceOptions bindCmd = new CbuCommand.BindServiceOptions(
                    cbuId, actor, UUID.fromString("15244192-0e29-4cd4-8d3b-ec19488ad814"), optServiceId, null, optVersionId, null,
                    "{\"opt_key\": \"val1\"}", null, null, null
                );
                CbuExecutor.execute(conn, bindCmd);
                long javaBindingCount = 0;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaBindingCount = rs.getLong(1);
                    }
                }
                assertEquals(rustBindingCount, javaBindingCount);

                // ------------------ 13. cbu.override-option-binding ------------------
                executeRustDsl(String.format(
                    "(cbu.override-option-binding :cbu-id \"%s\" :service-id \"%s\" :service-option-def-id \"%s\" :value \"val2\")",
                    cbuId, optServiceId, optDefId
                ));
                String rustVal = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT value::text FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustVal = rs.getString(1);
                    }
                }
                assertEquals("\"val2\"", rustVal);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();
                CbuExecutor.execute(conn, bindCmd);
                conn.commit();

                // Java Override
                CbuCommand.OverrideOptionBinding overrideCmd = new CbuCommand.OverrideOptionBinding(
                    cbuId, actor, optServiceId, null, optDefId, null, "\"val2\"", null, null, null, null
                );
                CbuExecutor.execute(conn, overrideCmd);
                String javaVal = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT value::text FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaVal = rs.getString(1);
                    }
                }
                assertEquals(rustVal, javaVal);

                // ------------------ 14. cbu.dirty-flag-bindings ------------------
                executeRustDsl(String.format(
                    "(cbu.dirty-flag-bindings :cbu-id \"%s\" :service-id \"%s\")",
                    cbuId, optServiceId
                ));
                String rustCoherence = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT coherence_status FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustCoherence = rs.getString(1);
                    }
                }
                assertEquals("dirty", rustCoherence);

                // Reset
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_service_option_bindings SET coherence_status = 'clean' WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Dirty Flag
                CbuCommand.DirtyFlagBindings dirtyCmd = new CbuCommand.DirtyFlagBindings(cbuId, actor, optServiceId, "manual");
                CbuExecutor.execute(conn, dirtyCmd);
                String javaCoherence = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT coherence_status FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaCoherence = rs.getString(1);
                    }
                }
                assertEquals(rustCoherence, javaCoherence);

                // ------------------ 15. cbu.recompute-bindings ------------------
                // Make dirty first
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_service_option_bindings SET coherence_status = 'dirty' WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    ps.executeUpdate();
                }
                conn.commit();

                executeRustDsl(String.format(
                    "(cbu.recompute-bindings :cbu-id \"%s\" :service-id \"%s\")",
                    cbuId, optServiceId
                ));
                String rustCoherence2 = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT coherence_status FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustCoherence2 = rs.getString(1);
                    }
                }
                assertEquals("clean", rustCoherence2);

                // Make dirty again
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_service_option_bindings SET coherence_status = 'dirty' WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java Recompute
                CbuCommand.RecomputeBindings recomputeCmd = new CbuCommand.RecomputeBindings(cbuId, actor, optServiceId);
                CbuExecutor.execute(conn, recomputeCmd);
                String javaCoherence2 = null;
                try (PreparedStatement ps = conn.prepareStatement("SELECT coherence_status FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ? AND service_id = ? AND valid_to IS NULL")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, optServiceId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaCoherence2 = rs.getString(1);
                    }
                }
                assertEquals(rustCoherence2, javaCoherence2);

                // ------------------ 16. cbu.delete-cascade ------------------
                // Re-assign role so we have roles to delete-cascade
                CbuExecutor.execute(conn, assignOwnership);
                // Add structure link to cascade delete
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_structure_links (link_id, parent_cbu_id, child_cbu_id, relationship_type, relationship_selector, status) VALUES (?, ?, ?, 'FEEDER', 'feeder:us', 'ACTIVE')")) {
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, cbuId);
                    ps.setObject(3, childCbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                executeRustDsl(String.format(
                    "(cbu.delete-cascade :cbu-id \"%s\" :delete-entities true :hard-delete false)",
                    cbuId
                ));
                // Verify deleted
                long rustActiveLinks = 1;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_structure_links WHERE parent_cbu_id = ? AND status = 'ACTIVE'")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustActiveLinks = rs.getLong(1);
                    }
                }
                assertEquals(0, rustActiveLinks);

                // Assert scope of Rust delete-cascade:
                // 1. Child CBU must NOT be deleted
                CbuStatus rustChild = CbuRepository.recover(conn, childCbuId);
                assertNotNull(rustChild);
                // 2. Parent CBU itself IS deleted (deleted_at is NOT null)
                boolean rustParentIsDeleted = false;
                try (PreparedStatement ps = conn.prepareStatement("SELECT deleted_at IS NOT NULL FROM \"ob-poc\".cbus WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) rustParentIsDeleted = rs.getBoolean(1);
                    }
                }
                assertTrue(rustParentIsDeleted);

                // Reset CBU and links and roles back to active
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbus SET deleted_at = null WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_structure_links SET status = 'ACTIVE', terminated_at = null, terminated_reason = null WHERE parent_cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("UPDATE \"ob-poc\".cbu_entity_roles SET effective_to = null WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();

                // Java DeleteCascade
                CbuCommand.DeleteCascade deleteCascadeCmd = new CbuCommand.DeleteCascade(
                    cbuId, actor, true, false
                );
                CbuExecutor.execute(conn, deleteCascadeCmd);

                long javaActiveLinks = 1;
                try (PreparedStatement ps = conn.prepareStatement("SELECT COUNT(*) FROM \"ob-poc\".cbu_structure_links WHERE parent_cbu_id = ? AND status = 'ACTIVE'")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaActiveLinks = rs.getLong(1);
                    }
                }
                assertEquals(rustActiveLinks, javaActiveLinks);
                assertEquals(0, javaActiveLinks);

                // Assert scope of Java delete-cascade:
                // 1. Child CBU must NOT be deleted
                CbuStatus javaChild = CbuRepository.recover(conn, childCbuId);
                assertNotNull(javaChild);
                // 2. Parent CBU itself IS deleted (deleted_at is NOT null)
                boolean javaParentIsDeleted = false;
                try (PreparedStatement ps = conn.prepareStatement("SELECT deleted_at IS NOT NULL FROM \"ob-poc\".cbus WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    try (ResultSet rs = ps.executeQuery()) {
                        if (rs.next()) javaParentIsDeleted = rs.getBoolean(1);
                    }
                }
                assertTrue(javaParentIsDeleted);

            } finally {
                try {
                    conn.rollback();
                } catch (Exception ignored) {}
                // Cleanup
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_corporate_action_events WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".service_option_defs WHERE option_key = 'opt_key'")) {
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".service_versions WHERE version = 'v1' AND service_id IN (SELECT service_id FROM \"ob-poc\".services WHERE name = 'Options test service')")) {
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".services WHERE name = 'Options test service'")) {
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
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".client_group_entity WHERE group_id = ?")) {
                    ps.setObject(1, groupId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_group_members WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_groups WHERE group_id = ?")) {
                    ps.setObject(1, groupId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".client_group WHERE id = ?")) {
                    ps.setObject(1, groupId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id IN (?, ?)")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, childCbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".entity_relationships WHERE from_entity_id IN (?, ?, ?) OR to_entity_id IN (?, ?, ?)")) {
                    ps.setObject(1, entityId1);
                    ps.setObject(2, entityId2);
                    ps.setObject(3, mancoEntityId);
                    ps.setObject(4, entityId1);
                    ps.setObject(5, entityId2);
                    ps.setObject(6, mancoEntityId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".entities WHERE entity_id IN (?, ?, ?)")) {
                    ps.setObject(1, entityId1);
                    ps.setObject(2, entityId2);
                    ps.setObject(3, mancoEntityId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_structure_links WHERE parent_cbu_id = ? OR child_cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, childCbuId);
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

    @Test
    public void testPhase5ReadProjectionsDifferentialConformance() throws Exception {
        UUID cbuId = UUID.randomUUID();
        UUID childCbuId = UUID.randomUUID();
        UUID entityId1 = UUID.randomUUID();
        UUID entityId2 = UUID.randomUUID();
        String cbuName = "Test Phase5 Proj CBU " + cbuId;
        CbuCommand.Principal actor = new CbuCommand.Principal("test-user", Set.of("compliance_officer"));

        try (Connection conn = getDbConnection()) {
            conn.setAutoCommit(false);
            try {
                // 1. Create a Fund CBU
                CbuCommand.Create createParent = new CbuCommand.Create(
                    cbuId, actor, cbuName, "LU", null, null, "FUND", null, null, null
                );
                CbuExecutor.execute(conn, createParent);

                // --- TEST validate-roles ---
                // Without control role or MANCO, it should have issues
                CbuRoleValidationDto javaVal1 = CbuReadProjection.validateRoles(conn, cbuId);
                JsonNode rustVal1 = runRustQueryJson("(cbu.validate-roles :cbu-id \"" + cbuId + "\")");
                
                assertNotNull(javaVal1);
                assertEquals(rustVal1.get("valid").asBoolean(), javaVal1.valid());
                assertEquals(rustVal1.get("issues").size(), javaVal1.issues().size());
                String rustCategory1 = (rustVal1.get("cbu_category") == null || rustVal1.get("cbu_category").isNull()) ? null : rustVal1.get("cbu_category").asText();
                String rustClientType1 = (rustVal1.get("client_type") == null || rustVal1.get("client_type").isNull()) ? null : rustVal1.get("client_type").asText();
                assertEquals(rustCategory1, javaVal1.cbuCategory());
                assertEquals(rustClientType1, javaVal1.clientType());

                // Now insert director and ManCo role to satisfy the validator
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".entities (entity_id, entity_type_id, name, name_norm, row_version) VALUES (?, ?, ?, ?, 1)")) {
                    ps.setObject(1, entityId1);
                    ps.setObject(2, UUID.fromString("6f7b4e87-363e-4ee3-a717-d4b456d4eec4")); // NAT_PERSON
                    ps.setString(3, "Director Jane " + entityId1.toString().substring(0,8));
                    ps.setString(4, ("Director Jane " + entityId1.toString().substring(0,8)).toLowerCase());
                    ps.executeUpdate();
                    
                    ps.setObject(1, entityId2);
                    ps.setObject(2, UUID.fromString("7803ffb7-935e-4cba-aa70-c9bb4cb43509")); // LIMITED_COMPANY_PRIVATE
                    ps.setString(3, "Manco Entity " + entityId2.toString().substring(0,8));
                    ps.setString(4, ("Manco Entity " + entityId2.toString().substring(0,8)).toLowerCase());
                    ps.executeUpdate();
                }

                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, version) VALUES (?, ?, ?, ?, 1)")) {
                    // Control role (DIRECTOR)
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, cbuId);
                    ps.setObject(3, entityId1);
                    ps.setObject(4, UUID.fromString("58e094e8-1531-42fb-9b70-8c936448d27c")); // DIRECTOR (CONTROL category)
                    ps.executeUpdate();

                    // MANAGEMENT_COMPANY
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, cbuId);
                    ps.setObject(3, entityId2);
                    ps.setObject(4, UUID.fromString("bd2a47cb-d961-4744-9985-1dd580957dbd")); // MANAGEMENT_COMPANY
                    ps.executeUpdate();
                }
                conn.commit();

                // Re-test validate-roles: should match Rust exactly
                CbuRoleValidationDto javaVal2 = CbuReadProjection.validateRoles(conn, cbuId);
                JsonNode rustVal2 = runRustQueryJson("(cbu.validate-roles :cbu-id \"" + cbuId + "\")");
                assertEquals(rustVal2.get("valid").asBoolean(), javaVal2.valid());
                assertEquals(rustVal2.get("issues").size(), javaVal2.issues().size());
                
                // Assert that the issues decreased by exactly 2 (the two roles we satisfied)
                assertEquals(javaVal1.issues().size() - 2, javaVal2.issues().size());
                
                String rustCategory2 = (rustVal2.get("cbu_category") == null || rustVal2.get("cbu_category").isNull()) ? null : rustVal2.get("cbu_category").asText();
                String rustClientType2 = (rustVal2.get("client_type") == null || rustVal2.get("client_type").isNull()) ? null : rustVal2.get("client_type").asText();
                assertEquals(rustCategory2, javaVal2.cbuCategory());
                assertEquals(rustClientType2, javaVal2.clientType());

                // --- TEST compute-resource-fanout ---
                // Let's seed a custom service-options rule and binding.
                UUID dummyServiceId = UUID.randomUUID();
                UUID dummyVersionId = UUID.randomUUID();
                UUID dummyDefId = UUID.randomUUID();
                UUID dummyRuleId = UUID.randomUUID();
                UUID dummyResourceId = UUID.fromString("4fd204b4-1c8f-4284-a61c-55618aedc522"); // DTCC Settlement System

                // Insert dummy service and components
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".services (service_id, service_code, name) VALUES (?, ?, ?)")) {
                    ps.setObject(1, dummyServiceId);
                    ps.setString(2, "DUMMY_SERVICE_" + dummyServiceId.toString().substring(0,8));
                    ps.setString(3, "Dummy service description");
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".service_versions (id, service_id, version, lifecycle_status) VALUES (?, ?, 'v1', 'published')")) {
                    ps.setObject(1, dummyVersionId);
                    ps.setObject(2, dummyServiceId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".service_option_defs (service_option_def_id, service_id, service_version_id, option_key, option_kind, is_required, lifecycle_status, default_source_kind) VALUES (?, ?, ?, ?, 'string', true, 'active', 'manual')")) {
                    ps.setObject(1, dummyDefId);
                    ps.setObject(2, dummyServiceId);
                    ps.setObject(3, dummyVersionId);
                    ps.setString(4, "dummy_option_key");
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".service_resource_fanout_rules (fanout_rule_id, service_id, resource_id, service_option_def_id, fanout_axis, fanout_mode, shared_when_null, priority, is_active) VALUES (?, ?, ?, ?, 'currency', 'per_value', false, 10, true)")) {
                    ps.setObject(1, dummyRuleId);
                    ps.setObject(2, dummyServiceId);
                    ps.setObject(3, dummyResourceId);
                    ps.setObject(4, dummyDefId);
                    ps.executeUpdate();
                }
                // Insert a binding for this option key: value is a JSON array ["USD", "EUR"]
                try (PreparedStatement ps = conn.prepareStatement("INSERT INTO \"ob-poc\".cbu_service_option_bindings (cbu_id, product_id, service_id, service_version_id, service_option_def_id, option_key, value, source_kind, value_hash, coherence_status) VALUES (?, ?, ?, ?, ?, ?, ?::jsonb, 'manual', 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 'clean')")) {
                    ps.setObject(1, cbuId);
                    ps.setObject(2, UUID.fromString("15244192-0e29-4cd4-8d3b-ec19488ad814")); // dummy product ID
                    ps.setObject(3, dummyServiceId);
                    ps.setObject(4, dummyVersionId);
                    ps.setObject(5, dummyDefId);
                    ps.setString(6, "dummy_option_key");
                    ps.setString(7, "[\"USD\",\"EUR\"]");
                    ps.executeUpdate();
                }
                conn.commit();

                // Compute fanout in Java
                List<CbuResourceFanoutDto> javaFanout = CbuReadProjection.computeResourceFanout(conn, cbuId, dummyServiceId, null);
                
                // Compute fanout in Rust
                JsonNode rustFanout = runRustQueryJson(String.format(
                    "(cbu.compute-resource-fanout :cbu-id \"%s\" :service-id \"%s\")",
                    cbuId, dummyServiceId
                ));

                assertNotNull(javaFanout);
                assertEquals(2, javaFanout.size());
                assertEquals(rustFanout.size(), javaFanout.size());

                for (int i = 0; i < javaFanout.size(); i++) {
                    CbuResourceFanoutDto jf = javaFanout.get(i);
                    JsonNode rf = rustFanout.get(i);
                    assertEquals(rf.get("service_id").asText(), jf.serviceId().toString());
                    assertEquals(rf.get("resource_id").asText(), jf.resourceId().toString());
                    assertEquals(rf.get("fanout_axis").asText(), jf.fanoutAxis());
                    String expectedVal = rf.get("fanout_value").toString();
                    assertEquals(expectedVal, jf.fanoutValue());
                }

            } finally {
                // Cleanup
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_service_option_bindings WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".service_resource_fanout_rules WHERE service_id IN (SELECT service_id FROM \"ob-poc\".services WHERE name = 'Dummy service description')")) {
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".service_option_defs WHERE service_id IN (SELECT service_id FROM \"ob-poc\".services WHERE name = 'Dummy service description')")) {
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".service_versions WHERE service_id IN (SELECT service_id FROM \"ob-poc\".services WHERE name = 'Dummy service description')")) {
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".services WHERE name = 'Dummy service description'")) {
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".entities WHERE entity_id IN (?, ?)")) {
                    ps.setObject(1, entityId1);
                    ps.setObject(2, entityId2);
                    ps.executeUpdate();
                }
                try (PreparedStatement ps = conn.prepareStatement("DELETE FROM \"ob-poc\".cbus WHERE cbu_id = ?")) {
                    ps.setObject(1, cbuId);
                    ps.executeUpdate();
                }
                conn.commit();
            }
        }
    }
}

