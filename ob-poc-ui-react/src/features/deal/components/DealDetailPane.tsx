/**
 * Deal Detail Pane - Shows details for selected node in deal taxonomy
 */

import { useQuery } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { dealApi } from "../../../api/deal";
import { queryKeys } from "../../../lib/query";
import type {
  DealTaxonomyNode,
  DealSummary,
  DealProductSummary,
  RateCardSummary,
  DealParticipantSummary,
  DealContractSummary,
  OnboardingRequestSummary,
} from "../../../types/deal";

interface DealDetailPaneProps {
  node: DealTaxonomyNode | null;
}

/** Field display component */
function Field({
  label,
  value,
}: {
  label: string;
  value?: string | number | null;
}) {
  if (value === undefined || value === null) return null;
  return (
    <div className="py-2 border-b border-[var(--border-secondary)]">
      <dt className="text-xs text-[var(--text-muted)] uppercase tracking-wide">
        {label}
      </dt>
      <dd className="mt-1 text-sm text-[var(--text-primary)]">{value}</dd>
    </div>
  );
}

/** Status badge */
function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    ACTIVE: "bg-green-500/20 text-green-400",
    PENDING: "bg-yellow-500/20 text-yellow-400",
    DRAFT: "bg-blue-500/20 text-blue-400",
    COMPLETED: "bg-green-500/20 text-green-400",
    SUBMITTED: "bg-blue-500/20 text-blue-400",
    REJECTED: "bg-red-500/20 text-red-400",
    EXPIRED: "bg-gray-500/20 text-gray-400",
  };

  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${colors[status] || "bg-gray-500/20 text-gray-400"}`}
    >
      {status}
    </span>
  );
}

/** Deal details */
function DealDetail({ deal }: { deal: DealSummary }) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          {deal.deal_name}
        </h2>
        <StatusBadge status={deal.deal_status} />
      </div>
      <dl>
        <Field label="Deal ID" value={deal.deal_id} />
        <Field label="Client Group" value={deal.client_group_name} />
        <Field label="Products" value={deal.product_count} />
        <Field label="Rate Cards" value={deal.rate_card_count} />
        <Field label="Participants" value={deal.participant_count} />
        <Field label="Contracts" value={deal.contract_count} />
        <Field
          label="Onboarding Requests"
          value={deal.onboarding_request_count}
        />
      </dl>
    </div>
  );
}

/** Product details */
function ProductDetail({ product }: { product: DealProductSummary }) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          {product.product_name}
        </h2>
        <StatusBadge status={product.product_status} />
      </div>
      <dl>
        <Field label="Product ID" value={product.deal_product_id} />
        <Field label="Product Code" value={product.product_code} />
        <Field label="Category" value={product.product_category} />
        <Field label="Rate Cards" value={product.rate_card_count} />
      </dl>
    </div>
  );
}

/** Rate card details with lines */
function RateCardDetail({ rateCard }: { rateCard: RateCardSummary }) {
  const { data: lines, isLoading } = useQuery({
    queryKey: queryKeys.deals.rateCardLines(rateCard.rate_card_id),
    queryFn: () => dealApi.getRateCardLines(rateCard.rate_card_id),
  });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          {rateCard.rate_card_name}
        </h2>
        {rateCard.status && <StatusBadge status={rateCard.status} />}
      </div>
      <dl>
        <Field label="Rate Card ID" value={rateCard.rate_card_id} />
        <Field label="Effective From" value={rateCard.effective_from} />
        <Field label="Effective To" value={rateCard.effective_to} />
        <Field label="Line Count" value={rateCard.line_count} />
      </dl>

      {/* Rate card lines table */}
      <div className="mt-6">
        <h3 className="text-sm font-medium text-[var(--text-secondary)] mb-2">
          Rate Card Lines
        </h3>
        {isLoading ? (
          <div className="flex justify-center py-4">
            <Loader2 className="h-5 w-5 animate-spin text-[var(--text-muted)]" />
          </div>
        ) : lines && lines.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-[var(--border-secondary)]">
                  <th className="px-2 py-2 text-left text-xs text-[var(--text-muted)]">
                    Fee Type
                  </th>
                  <th className="px-2 py-2 text-left text-xs text-[var(--text-muted)]">
                    Subtype
                  </th>
                  <th className="px-2 py-2 text-left text-xs text-[var(--text-muted)]">
                    Model
                  </th>
                  <th className="px-2 py-2 text-right text-xs text-[var(--text-muted)]">
                    Rate
                  </th>
                  <th className="px-2 py-2 text-left text-xs text-[var(--text-muted)]">
                    Currency
                  </th>
                </tr>
              </thead>
              <tbody>
                {lines.map((line) => (
                  <tr
                    key={line.line_id}
                    className="border-b border-[var(--border-secondary)] hover:bg-[var(--bg-hover)]"
                  >
                    <td className="px-2 py-2 text-[var(--text-primary)]">
                      {line.fee_type}
                    </td>
                    <td className="px-2 py-2 text-[var(--text-secondary)]">
                      {line.fee_subtype}
                    </td>
                    <td className="px-2 py-2 text-[var(--text-secondary)]">
                      {line.pricing_model}
                    </td>
                    <td className="px-2 py-2 text-right text-[var(--text-primary)] tabular-nums">
                      {line.rate_value || "-"}
                    </td>
                    <td className="px-2 py-2 text-[var(--text-secondary)]">
                      {line.currency || "-"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="text-sm text-[var(--text-muted)]">No lines defined</p>
        )}
      </div>
    </div>
  );
}

/** Participant details */
function ParticipantDetail({
  participant,
}: {
  participant: DealParticipantSummary;
}) {
  return (
    <div className="space-y-4">
      <h2 className="text-lg font-semibold text-[var(--text-primary)]">
        {participant.entity_name}
      </h2>
      <dl>
        <Field label="Participant ID" value={participant.participant_id} />
        <Field label="Entity ID" value={participant.entity_id} />
        <Field label="Role" value={participant.role} />
        <Field label="Jurisdiction" value={participant.jurisdiction} />
        <Field label="LEI" value={participant.lei} />
      </dl>
    </div>
  );
}

/** Contract details */
function ContractDetail({ contract }: { contract: DealContractSummary }) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          {contract.contract_name}
        </h2>
        <StatusBadge status={contract.status} />
      </div>
      <dl>
        <Field label="Contract ID" value={contract.contract_id} />
        <Field label="Contract Type" value={contract.contract_type} />
        <Field label="Effective Date" value={contract.effective_date} />
        <Field label="Termination Date" value={contract.termination_date} />
      </dl>
    </div>
  );
}

/** Onboarding request details */
function OnboardingDetail({ request }: { request: OnboardingRequestSummary }) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          {request.request_type}
        </h2>
        <StatusBadge status={request.status} />
      </div>
      <dl>
        <Field label="Request ID" value={request.request_id} />
        <Field label="CBU" value={request.cbu_name} />
        <Field label="Submitted At" value={request.submitted_at} />
        <Field label="Completed At" value={request.completed_at} />
      </dl>
    </div>
  );
}

export function DealDetailPane({ node }: DealDetailPaneProps) {
  if (!node) {
    return (
      <div className="flex h-full items-center justify-center text-[var(--text-muted)]">
        <p>Select a node to view details</p>
      </div>
    );
  }

  // List nodes don't have details, show placeholder
  if (
    node.type === "product_list" ||
    node.type === "participant_list" ||
    node.type === "contract_list" ||
    node.type === "onboarding_list" ||
    node.type === "rate_card_list"
  ) {
    return (
      <div className="p-4">
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          {node.label}
        </h2>
        <p className="mt-2 text-sm text-[var(--text-muted)]">
          Select an item from the list to view details.
        </p>
      </div>
    );
  }

  return (
    <div className="p-4">
      {node.type === "deal" && node.data && (
        <DealDetail deal={node.data as DealSummary} />
      )}
      {node.type === "product" && node.data && (
        <ProductDetail product={node.data as DealProductSummary} />
      )}
      {node.type === "rate_card" && node.data && (
        <RateCardDetail rateCard={node.data as RateCardSummary} />
      )}
      {node.type === "participant" && node.data && (
        <ParticipantDetail participant={node.data as DealParticipantSummary} />
      )}
      {node.type === "contract" && node.data && (
        <ContractDetail contract={node.data as DealContractSummary} />
      )}
      {node.type === "onboarding" && node.data && (
        <OnboardingDetail request={node.data as OnboardingRequestSummary} />
      )}
    </div>
  );
}

export default DealDetailPane;
