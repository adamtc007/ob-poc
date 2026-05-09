import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ChatMessage } from "./ChatMessage";
import type { ChatMessage as ChatMessageType } from "../../../types/chat";

describe("ChatMessage ACP trace rendering", () => {
  it("renders structured ACP trace details for HIL review", () => {
    const message: ChatMessageType = {
      id: "msg-1",
      role: "assistant",
      content: "I stopped before executing a dry-run.",
      timestamp: "2026-05-09T12:00:00Z",
      acp_trace: {
        status: "structured_refusal",
        outcome: "structured_refusal",
        outcome_layer: "validation_refusal",
        human_summary: "I stopped because evidence is missing.",
        refusal_code: "missing_evidence_digest",
        needed_from_user: ["evidence_digest"],
        diagnostic_codes: ["missing_evidence_digest"],
        dry_run_valid: false,
        prose_only_failure: false,
        revision_count: 0,
      },
    };

    render(<ChatMessage message={message} />);

    expect(screen.getByText("ACP Trace")).toBeInTheDocument();
    expect(screen.getByText("structured_refusal")).toBeInTheDocument();
    expect(screen.getByText("layer: validation_refusal")).toBeInTheDocument();
    expect(screen.getByText("refusal: missing_evidence_digest")).toBeInTheDocument();
    expect(screen.getByText("evidence digest")).toBeInTheDocument();
    expect(screen.getByText("missing_evidence_digest")).toBeInTheDocument();
    expect(screen.getByText("dry-run: not valid")).toBeInTheDocument();
    expect(screen.getByText("prose-only failure: no")).toBeInTheDocument();
  });
});
