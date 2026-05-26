/**
 * FormioForm — renders a Form.io form schema and handles submission.
 *
 * This is the JS half of the dsl.form verb boundary:
 *   - Receives {formRef, prefillData, mode, tokenId} from the session response
 *   - Fetches the form schema via GET /api/forms/:ref
 *   - Renders via Form.io SDK (Formio.createForm)
 *   - Prefills: instance.submission = { data: prefillData }
 *   - On submit: POST /api/forms/:tokenId/submit → triggers HumanTaskComplete
 *
 * mode = "display":  form is read-only with a single Continue button
 * mode = "capture":  form is editable, submit captures all field values
 */

import React, { useEffect, useRef, useState } from "react";
import { fetchFormSchema, submitForm, FormSchema } from "../../api/forms";

export interface FormioFormProps {
  formRef: string;
  prefillData: Record<string, unknown>;
  mode: "display" | "capture";
  tokenId: string;
  onComplete?: (submissionData: Record<string, unknown>) => void;
}

export function FormioForm({
  formRef,
  prefillData,
  mode,
  tokenId,
  onComplete,
}: FormioFormProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [schema, setSchema] = useState<FormSchema | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const formInstanceRef = useRef<unknown>(null);

  // Fetch schema on mount
  useEffect(() => {
    fetchFormSchema(formRef)
      .then(setSchema)
      .catch((e: Error) => setError(e.message))
      .finally(() => setLoading(false));
  }, [formRef]);

  // Render form when schema is ready
  useEffect(() => {
    if (!schema || !containerRef.current) return;

    let destroyed = false;

    // Dynamic import of formiojs so the SDK is only loaded when needed
    import("formiojs")
      .then(({ Formio }) => {
        if (destroyed || !containerRef.current) return;

        // Make display-mode forms read-only
        const options =
          mode === "display"
            ? { readOnly: true, viewAsHtml: false }
            : {};

        Formio.createForm(containerRef.current, schema, options).then(
          (instance: { submission: unknown; on: (event: string, cb: (submission: unknown) => void) => void; destroy: () => void }) => {
            formInstanceRef.current = instance;

            // Prefill with process context data
            if (prefillData && Object.keys(prefillData).length > 0) {
              instance.submission = { data: prefillData };
            }

            // Handle submission
            instance.on("submit", async (submission: unknown) => {
              const data =
                (submission as { data?: Record<string, unknown> })?.data ?? {};
              setSubmitting(true);
              try {
                await submitForm(tokenId, data);
                onComplete?.(data);
              } catch (e) {
                setError(`Submission failed: ${(e as Error).message}`);
              } finally {
                setSubmitting(false);
              }
            });
          },
        );
      })
      .catch(() => {
        setError(
          "Form.io SDK not available. Install formiojs: npm install formiojs",
        );
      });

    return () => {
      destroyed = true;
      if (
        formInstanceRef.current &&
        typeof (formInstanceRef.current as { destroy?: () => void }).destroy ===
          "function"
      ) {
        (formInstanceRef.current as { destroy: () => void }).destroy();
        formInstanceRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [schema]);

  if (loading) {
    return (
      <div className="p-4 text-sm text-gray-500">Loading form…</div>
    );
  }

  if (error) {
    return (
      <div className="p-4 text-sm text-red-600 bg-red-50 rounded border border-red-200">
        <strong>Form error:</strong> {error}
      </div>
    );
  }

  return (
    <div className="relative">
      {submitting && (
        <div className="absolute inset-0 bg-white/70 flex items-center justify-center z-10">
          <span className="text-sm text-gray-500">Submitting…</span>
        </div>
      )}
      <div ref={containerRef} className="formio-form" />
    </div>
  );
}
