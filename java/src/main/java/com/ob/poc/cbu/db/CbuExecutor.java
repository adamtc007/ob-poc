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
                UUID existingFundCbuId = null;
                boolean isLinked = false;
                if (cmd.fundEntityId() != null) {
                    existingFundCbuId = CbuRepository.getLinkedCbuIdForFund(conn, cmd.fundEntityId());
                    isLinked = (existingFundCbuId != null);
                }
                context = new CbuDecide.DecisionContext(isLinked, existingFundCbuId);
            } else if (command instanceof CbuCommand.Ensure cmd) {
                status = CbuRepository.recoverByNameAndJurisdiction(conn, cmd.name(), cmd.jurisdiction());
            } else if (command instanceof CbuCommand.LinkStructure cmd) {
                boolean parentExists = CbuRepository.recover(conn, cmd.parentCbuId()) != null;
                boolean childExists = CbuRepository.recover(conn, cmd.childCbuId()) != null;
                UUID existingLinkId = null;
                if (parentExists && childExists) {
                    existingLinkId = CbuRepository.existingLinkId(conn, cmd.parentCbuId(), cmd.childCbuId(), cmd.relationshipType());
                }
                context = new CbuDecide.DecisionContext(parentExists, childExists, existingLinkId);
                status = CbuRepository.recover(conn, cmd.parentCbuId());
            } else if (command instanceof CbuCommand.UnlinkStructure cmd) {
                boolean activeOnly = cmd.hardDelete() == null || !cmd.hardDelete();
                boolean exists = CbuRepository.linkExists(conn, cmd.linkId(), activeOnly);
                context = CbuDecide.DecisionContext.forUnlink(exists);
                status = null;
            } else if (command instanceof CbuCommand.TerminateRole cmd) {
                boolean activeOnly = cmd.hardDelete() == null || !cmd.hardDelete();
                boolean exists = CbuRepository.roleExists(conn, cmd.cbuId(), activeOnly);
                context = CbuDecide.DecisionContext.forRole(exists);
                status = CbuRepository.recover(conn, cmd.cbuId());
            } else if (command instanceof CbuCommand.RemoveMember cmd) {
                boolean activeOnly = cmd.hardDelete() == null || !cmd.hardDelete();
                boolean exists = CbuRepository.memberExists(conn, cmd.cbuId(), activeOnly);
                context = CbuDecide.DecisionContext.forMember(exists);
                status = CbuRepository.recover(conn, cmd.cbuId());
            } else if (command instanceof CbuCommand.SubmitForReview 
                       || command instanceof CbuCommand.ApproveCa 
                       || command instanceof CbuCommand.RejectCa 
                       || command instanceof CbuCommand.WithdrawCa 
                       || command instanceof CbuCommand.MarkImplementedCa) {
                String caStatus = CbuRepository.recoverCaStatus(conn, command.id());
                context = CbuDecide.DecisionContext.forCa(caStatus);
            } else if (command instanceof CbuCommand.VerifyEvidence cmd) {
                // Evidence verify
                status = null;
            } else if (command instanceof CbuCommand.CreateFromClientGroup cmd) {
                status = null;
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
            if (command instanceof CbuCommand.Create cmd) {
                if (status != null) {
                    cbuId = status.id();
                } else if (context != null && context.isFundLinkedAlready() && context.existingFundCbuId() != null) {
                    cbuId = context.existingFundCbuId();
                }
            }
            boolean created = false;

            for (Effect effect : accept.effects()) {
                int affectedRows = CbuRepository.applyEffect(conn, effect, outEvents);
                if (effect instanceof Effect.UpdateOperationalStatus 
                    || effect instanceof Effect.UpdateValidationStatus 
                    || effect instanceof Effect.UpdateDispositionStatus) {
                    if (affectedRows == 0) {
                        conn.rollback();
                        return new CbuExecutionResult.Failure("concurrent modification / precondition no longer holds");
                    }
                }
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
