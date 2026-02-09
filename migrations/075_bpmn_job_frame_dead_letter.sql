-- Migration 075: Add 'dead_lettered' status to bpmn_job_frames
--
-- Jobs that exceed max_retries are promoted to the dead-letter queue.
-- This adds the status value to the CHECK constraint.

-- Drop and re-add the CHECK constraint to include 'dead_lettered'.
ALTER TABLE "ob-poc".bpmn_job_frames
    DROP CONSTRAINT IF EXISTS bpmn_job_frames_status_check;

ALTER TABLE "ob-poc".bpmn_job_frames
    ADD CONSTRAINT bpmn_job_frames_status_check
    CHECK (status IN ('active', 'completed', 'failed', 'dead_lettered'));
