-- Removed tools: meals, habits and contacts.
-- Drop FTS/content and child tables before their referents.
DROP TABLE IF EXISTS contacts_fts;
DROP TABLE IF EXISTS contacts;
DROP TABLE IF EXISTS meal_plans;
DROP TABLE IF EXISTS shopping_list;
DROP TABLE IF EXISTS habit_logs;
DROP TABLE IF EXISTS habits;

-- Remove the seed rows of the removed tools.
DELETE FROM tools WHERE name IN ('meals', 'habits', 'contacts');
