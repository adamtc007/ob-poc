/**
 * Forms API client — Form.io verb integration.
 *
 * Two endpoints:
 *   GET  /api/forms/:ref          — fetch form schema JSON by ref key
 *   POST /api/forms/:tokenId/submit — deliver form submission to parked fiber
 */

import { api } from "./client";

/** A Form.io form schema as returned by the server. */
export type FormSchema = Record<string, unknown>;

/** Fetch a form schema by its ref key. */
export async function fetchFormSchema(formRef: string): Promise<FormSchema> {
  return api.get<FormSchema>(`/forms/${encodeURIComponent(formRef)}`);
}

/** Submit a form response for the given parked fiber token. */
export async function submitForm(
  tokenId: string,
  submissionData: Record<string, unknown>,
): Promise<void> {
  await api.post<unknown>(`/forms/${encodeURIComponent(tokenId)}/submit`, submissionData);
}
