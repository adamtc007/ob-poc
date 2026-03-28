-- Add phrase_mapping to the sem_reg.object_type enum
ALTER TYPE sem_reg.object_type ADD VALUE IF NOT EXISTS 'phrase_mapping';
