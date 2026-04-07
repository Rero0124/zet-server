-- Add username (handle) column to users
ALTER TABLE users ADD COLUMN username VARCHAR(30) UNIQUE;

-- Backfill existing users: use email prefix as default username
UPDATE users SET username = split_part(email, '@', 1) || '_' || substr(id::text, 1, 4);

-- Now make it NOT NULL
ALTER TABLE users ALTER COLUMN username SET NOT NULL;
