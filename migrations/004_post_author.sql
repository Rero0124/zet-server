-- Allow posts from regular users, not just companies
ALTER TABLE posts ADD COLUMN IF NOT EXISTS author_id UUID REFERENCES users(id);
ALTER TABLE posts ALTER COLUMN company_id DROP NOT NULL;

-- Backfill: set author_id from company owner for existing posts
UPDATE posts SET author_id = (SELECT user_id FROM companies WHERE companies.id = posts.company_id)
WHERE author_id IS NULL AND company_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_posts_author ON posts(author_id);
