import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { RunbookPlanReview } from "./RunbookPlanReview";
import { runbookPlanApi } from "../../api/runbookPlan";
import type {
  KycUpdateStatusDryRunResult,
  RunbookPlan,
} from "../../api/runbookPlan";

vi.mock("../../api/runbookPlan", () => ({
  runbookPlanApi: {
    getRunbookPlan: vi.fn(),
    approveRunbookPlan: vi.fn(),
    cancelRunbookPlan: vi.fn(),
    executeRunbookPlanStep: vi.fn(),
    getRunbookStatus: vi.fn(),
  },
}));

const apiMock = vi.mocked(runbookPlanApi);

function plan(status: RunbookPlan["status"] = { status: "compiled" }) {
  return {
    id: "plan-1234567890abcdef",
    session_id: "session-123",
    compiled_at: "2026-05-05T12:00:00Z",
    source_research: [1],
    steps: [
      {
        seq: 0,
        workspace: "kyc",
        constellation_map: "kyc_onboarding",
        subject_kind: "case",
        subject_binding: {
          kind: "literal",
          id: "case-123",
        },
        verb: {
          verb_fqn: "kyc-case.update-status",
          display_name: "Update status",
        },
        sentence: "Move the KYC case to assessment.",
        args: {
          case_id: "case-123",
          current_state: "DISCOVERY",
          requested_state: "ASSESSMENT",
        },
        preconditions: [],
        expected_effect: "Case enters assessment.",
        depends_on: [],
        status: "ready",
      },
    ],
    bindings: {
      entries: {},
      resolved: {},
    },
    status,
  } satisfies RunbookPlan;
}

function dryRunResult(): KycUpdateStatusDryRunResult {
  return {
    status: "dry_run_validated",
    workbook: {
      id: "ewb:v1:abc123",
      core: {
        schema_version: 1,
        pack_id: "ob-poc-kyc",
        transition_ref: "kyc-case.discovery-to-assessment",
        execution_mode: "dry_run",
        session_id: "session-123",
        subject: {
          subject_kind: "case",
          subject_id: "case-123",
        },
        actor: {
          actor_id: "agent-1",
          roles: ["ops"],
        },
        configuration_version: "sem-os-v1",
        state_snapshot_id: "snapshot-1",
        objective: "Move KYC case from discovery to assessment",
        editor_context_refs: ["semos://entity/case-123"],
        evidence_refs: [],
        expected_preconditions: ["status == DISCOVERY"],
        expected_postconditions: ["status == ASSESSMENT"],
        invariant_checks: [],
        governance_checks: [],
        simulation: {
          transition_ref: "kyc-case.discovery-to-assessment",
          entity_id: "case-123",
          entity_type: "kyc_case",
          state_machine: "kyc_case_lifecycle",
          from_state: "DISCOVERY",
          to_state: "ASSESSMENT",
          verb: "kyc-case.update-status",
          semantic_diff: {
            field: "status",
            before: "DISCOVERY",
            after: "ASSESSMENT",
          },
          predicted_advance: {
            entity_id: "case-123",
            to_node: "ASSESSMENT",
            slot_path: "case.status",
            reason: "configuration transition kyc-case.discovery-to-assessment",
            writes_since_push_delta: 0,
          },
          state_snapshot_id: "snapshot-1",
          configuration_version: "sem-os-v1",
        },
        stale_policy: "reject",
        metadata: {},
      },
      status: "validated",
      created_at: "2026-05-05T12:00:00Z",
    },
    dry_run: {
      workbook_id: "ewb:v1:abc123",
      transition_ref: "kyc-case.discovery-to-assessment",
      semantic_diff_uri: "semos://semantic-diff/ewb:v1:abc123",
      validation_trace: [
        {
          step_number: 3,
          step_id: "integrity",
          status: "passed",
          message: "workbook integrity hash verified",
        },
      ],
      semantic_diff: {
        transition_ref: "kyc-case.discovery-to-assessment",
        entity_id: "case-123",
        entity_type: "kyc_case",
        state_machine: "kyc_case_lifecycle",
        from_state: "DISCOVERY",
        to_state: "ASSESSMENT",
        verb: "kyc-case.update-status",
        semantic_diff: {
          field: "status",
          before: "DISCOVERY",
          after: "ASSESSMENT",
        },
        predicted_advance: {
          entity_id: "case-123",
          to_node: "ASSESSMENT",
          slot_path: "case.status",
          reason: "configuration transition kyc-case.discovery-to-assessment",
          writes_since_push_delta: 0,
        },
        state_snapshot_id: "snapshot-1",
        configuration_version: "sem-os-v1",
      },
    },
  };
}

describe("RunbookPlanReview", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the existing compiled-plan review actions", () => {
    render(<RunbookPlanReview sessionId="session-123" initialPlan={plan()} />);

    expect(screen.getByText("Runbook Plan")).toBeInTheDocument();
    expect(screen.getByText("kyc-case.update-status")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Approve" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: /Execute Next Step/i }),
    ).not.toBeInTheDocument();
  });

  it("approves through the existing runbook plan API", async () => {
    apiMock.approveRunbookPlan.mockResolvedValue({
      status: "approved",
      plan_id: "plan-1234567890abcdef",
    });
    apiMock.getRunbookPlan.mockResolvedValue(plan({ status: "approved" }));
    const onApproved = vi.fn();

    render(
      <RunbookPlanReview
        sessionId="session-123"
        initialPlan={plan()}
        onApproved={onApproved}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Approve" }));

    await waitFor(() => {
      expect(apiMock.approveRunbookPlan).toHaveBeenCalledWith("session-123");
      expect(onApproved).toHaveBeenCalledTimes(1);
    });
  });

  it("renders a validated workbook dry-run summary when provided", () => {
    render(
      <RunbookPlanReview
        sessionId="session-123"
        initialPlan={plan()}
        initialDryRunResult={dryRunResult()}
      />,
    );

    expect(screen.getByText("Workbook Dry Run")).toBeInTheDocument();
    expect(screen.getByText("ewb:v1:abc123")).toBeInTheDocument();
    expect(
      screen.getByText("kyc-case.discovery-to-assessment"),
    ).toBeInTheDocument();
    expect(screen.getAllByText(/DISCOVERY/)).toHaveLength(2);
    expect(screen.getAllByText(/ASSESSMENT/)).toHaveLength(2);
    expect(
      screen.getByText("semos://semantic-diff/ewb:v1:abc123"),
    ).toBeInTheDocument();
    expect(screen.getByText("1 checks, 1 passed")).toBeInTheDocument();
  });
});
