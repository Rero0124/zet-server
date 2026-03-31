-- Convert posts from plain text to block-based content
-- blocks: JSONB array of { "type": "text"|"image"|"video", "value": "..." }
ALTER TABLE posts ADD COLUMN IF NOT EXISTS blocks JSONB;

-- Migrate existing content to blocks format
UPDATE posts SET blocks = jsonb_build_array(jsonb_build_object('type', 'text', 'value', content))
WHERE blocks IS NULL AND content IS NOT NULL;

-- Update search vector trigger to extract text from blocks
CREATE OR REPLACE FUNCTION posts_search_update() RETURNS trigger AS $$
DECLARE
    text_content TEXT := '';
    block JSONB;
BEGIN
    IF NEW.blocks IS NOT NULL THEN
        FOR block IN SELECT * FROM jsonb_array_elements(NEW.blocks)
        LOOP
            IF block->>'type' = 'text' THEN
                text_content := text_content || ' ' || coalesce(block->>'value', '');
            END IF;
        END LOOP;
    ELSE
        text_content := coalesce(NEW.content, '');
    END IF;

    NEW.search_vector :=
        setweight(to_tsvector('simple', text_content), 'A') ||
        setweight(to_tsvector('simple', coalesce(NEW.category, '')), 'B') ||
        setweight(to_tsvector('simple', coalesce(array_to_string(NEW.tags, ' '), '')), 'B');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
