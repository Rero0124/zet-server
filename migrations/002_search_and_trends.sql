-- Full-text search index on posts
ALTER TABLE posts ADD COLUMN IF NOT EXISTS search_vector tsvector;

UPDATE posts SET search_vector =
    setweight(to_tsvector('simple', coalesce(content, '')), 'A') ||
    setweight(to_tsvector('simple', coalesce(category, '')), 'B') ||
    setweight(to_tsvector('simple', coalesce(array_to_string(tags, ' '), '')), 'B');

CREATE INDEX IF NOT EXISTS idx_posts_search ON posts USING gin(search_vector);

-- Trigger to keep search_vector up to date
CREATE OR REPLACE FUNCTION posts_search_update() RETURNS trigger AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('simple', coalesce(NEW.content, '')), 'A') ||
        setweight(to_tsvector('simple', coalesce(NEW.category, '')), 'B') ||
        setweight(to_tsvector('simple', coalesce(array_to_string(NEW.tags, ' '), '')), 'B');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS posts_search_trigger ON posts;
CREATE TRIGGER posts_search_trigger
    BEFORE INSERT OR UPDATE ON posts
    FOR EACH ROW EXECUTE FUNCTION posts_search_update();

-- Reaction counts on posts for quick access
ALTER TABLE posts ADD COLUMN IF NOT EXISTS like_count BIGINT NOT NULL DEFAULT 0;
ALTER TABLE posts ADD COLUMN IF NOT EXISTS review_count BIGINT NOT NULL DEFAULT 0;
ALTER TABLE posts ADD COLUMN IF NOT EXISTS bookmark_count BIGINT NOT NULL DEFAULT 0;
