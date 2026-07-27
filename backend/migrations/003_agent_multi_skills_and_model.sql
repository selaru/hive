-- Update agents table to support multiple skills and model selection

-- Add new column for model_name (selected model from provider)
ALTER TABLE agents ADD COLUMN IF NOT EXISTS model_name TEXT;

-- Add new column for skill_ids array (to replace skill_id)
ALTER TABLE agents ADD COLUMN IF NOT EXISTS skill_ids UUID[] NOT NULL DEFAULT '{}';

-- Copy existing skill_id to skill_ids array for migration (only if skill_id column exists)
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'agents' AND column_name = 'skill_id'
    ) THEN
        UPDATE agents SET skill_ids = ARRAY[skill_id] WHERE skill_id IS NOT NULL;
        
        -- Drop the old skill_id foreign key constraint and column
        ALTER TABLE agents DROP CONSTRAINT IF EXISTS agents_skill_id_fkey;
        ALTER TABLE agents DROP COLUMN skill_id;
    END IF;
END $$;

-- Add check constraint to ensure at least one skill is selected
ALTER TABLE agents DROP CONSTRAINT IF EXISTS agents_skill_ids_not_empty;
ALTER TABLE agents ADD CONSTRAINT agents_skill_ids_not_empty 
  CHECK (array_length(skill_ids, 1) > 0);
