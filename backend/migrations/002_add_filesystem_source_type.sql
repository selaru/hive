-- Add 'filesystem' as a valid source_type for skills

ALTER TABLE skills
DROP CONSTRAINT IF EXISTS skills_source_type_check;

ALTER TABLE skills
ADD CONSTRAINT skills_source_type_check
CHECK (source_type IN ('github', 'skills_sh', 'local', 'filesystem'));
