/**
 * FormioForm — renders a Form.io form schema and handles submission.
 *
 * JS half of the dsl.form verb boundary:
 *   - Fetches form schema via GET /api/forms/:ref
 *   - Renders via @formio/react <Form> component
 *   - Prefills with process context data
 *   - On submit: POST /api/forms/:tokenId/submit → HumanTaskComplete
 *
 * mode = "display":  read-only, single Continue acknowledgement button
 * mode = "capture":  editable, submit captures all field values
 */

import React, { useState } from "react";
import { Form } from "@formio/react";
import type { FormType, Submission } from "@formio/react/lib/components/Form";
import { fetchFormSchema, submitForm } from "../../api/forms";
import type { FormSchema } from "../../api/forms";

export interface FormioFormProps {
  formRef: string;
  prefillData: Record<string, unknown>;
  mode: "display" | "capture";
  tokenId: string;
  onComplete?: (submissionData: Record<string, unknown>) => void;
}

function useFormSchema(formRef: string) {
  const [schema, setSchema] = React.useState<FormSchema | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    setLoading(true);
    setError(null);
    fetchFormSchema(formRef)
      .then(setSchema)
      .catch((e: Error) => setError(e.message))
      .finally(() => setLoading(false));
  }, [formRef]);

  return { schema, loading, error };
}

export function FormioForm({
  formRef,
  prefillData,
  mode,
  tokenId,
  onComplete,
}: FormioFormProps) {
  const { schema, loading, error: schemaError } = useFormSchema(formRef);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  const handleSubmit = async (submission: Submission) => {
    setSubmitting(true);
    setSubmitError(null);
    const data = submission.data as Record<string, unknown>;
    try {
      await submitForm(tokenId, data);
      onComplete?.(data);
    } catch (e) {
      setSubmitError(`Submission failed: ${(e as Error).message}`);
    } finally {
      setSubmitting(false);
    }
  };

  if (loading) {
    return <div className="p-4 text-sm text-gray-500">Loading form…</div>;
  }

  if (schemaError) {
    return (
      <div className="p-4 text-sm text-red-600 bg-red-50 rounded border border-red-200">
        <strong>Form error:</strong> {schemaError}
      </div>
    );
  }

  if (!schema) return null;

  return (
    <div className="relative">
      {submitting && (
        <div className="absolute inset-0 bg-white/70 flex items-center justify-center z-10">
          <span className="text-sm text-gray-500">Submitting…</span>
        </div>
      )}
      {submitError && (
        <div className="mb-2 p-2 text-sm text-red-600 bg-red-50 rounded border border-red-200">
          {submitError}
        </div>
      )}
      <Form
        src={schema as unknown as FormType}
        // prefillData is Record<string,unknown> from the API; Form.io's JSON
        // recursive type is structurally identical at runtime — cast via unknown.
        submission={{ data: prefillData } as unknown as Submission}
        options={{ readOnly: mode === "display" }}
        onSubmit={handleSubmit}
      />
    </div>
  );
}
