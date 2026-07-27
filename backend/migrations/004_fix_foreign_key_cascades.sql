-- Migration: Fix foreign key cascades for deleting teams, agents, and team members safely
-- 
-- 1. Alter team_members to add ON DELETE CASCADE on agent_id
ALTER TABLE team_members 
  DROP CONSTRAINT IF EXISTS team_members_agent_id_fkey,
  ADD CONSTRAINT team_members_agent_id_fkey 
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE;

-- 2. Alter projects to add ON DELETE CASCADE on team_id
ALTER TABLE projects 
  DROP CONSTRAINT IF EXISTS projects_team_id_fkey,
  ADD CONSTRAINT projects_team_id_fkey 
    FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE CASCADE;

-- 3. Alter stories to add ON DELETE SET NULL on assigned_member_id
ALTER TABLE stories 
  DROP CONSTRAINT IF EXISTS stories_assigned_member_id_fkey,
  ADD CONSTRAINT stories_assigned_member_id_fkey 
    FOREIGN KEY (assigned_member_id) REFERENCES team_members(id) ON DELETE SET NULL;
