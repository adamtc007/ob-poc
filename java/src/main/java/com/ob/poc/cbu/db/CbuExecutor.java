package com.ob.poc.cbu.db;

import com.ob.poc.cbu.model.CbuCommand;
import com.ob.poc.cbu.model.CbuStatus;
import com.ob.poc.cbu.model.CbuDecide;
import com.ob.poc.cbu.model.DecisionOutcome;
import com.ob.poc.cbu.model.CbuExecutionResult;
import com.ob.poc.cbu.model.Effect;

import java.sql.Connection;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

public final class CbuExecutor {

    private CbuExecutor() {}

    public static CbuExecutionResult execute(Connection conn, CbuCommand command) throws SQLException {
        boolean originalAutoCommit = conn.getAutoCommit();
        try {
            conn.setAutoCommit(false);

            // 1. Recover state and context
            CbuStatus status = null;
            CbuDecide.DecisionContext context = null;

            if (command instanceof CbuCommand.Create cmd) {
                status = CbuRepository.recoverByNameAndJurisdiction(conn, cmd.name(), cmd.jurisdiction());
                boolean isLinked = cmd.fundEntityId() != null 
                    && CbuRepository.isFundLinkedAlready(conn, cmd.fundEntityId());
                context = new CbuDecide.DecisionContext(isLinked);
            } else {
                status = CbuRepository.recover(conn, command.id());
            }

            // 2. Decide
            DecisionOutcome outcome = CbuDecide.decide(command, status, context);

            // 3. Apply
            if (outcome instanceof DecisionOutcome.Refuse refuse) {
                conn.rollback();
                return new CbuExecutionResult.Failure(refuse.reason());
            }

            DecisionOutcome.Accept accept = (DecisionOutcome.Accept) outcome;
            List<Object> outEvents = new ArrayList<>();
            UUID cbuId = command.id();
            boolean created = false;

            for (Effect effect : accept.effects()) {
                CbuRepository.applyEffect(conn, effect, outEvents);
            }

            // Extract insert result if present
            for (Object event : outEvents) {
                if (event instanceof CbuRepository.InsertResult ir) {
                    cbuId = ir.cbuId();
                    created = ir.created();
                }
            }

            conn.commit();
            return new CbuExecutionResult.Success(cbuId, created, outEvents);

        } catch (Exception e) {
            conn.rollback();
            throw e;
        } finally {
            conn.setAutoCommit(originalAutoCommit);
        }
    }
}
