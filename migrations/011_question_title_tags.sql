-- questions에 제목, 태그 추가 (게시글 스타일)
ALTER TABLE questions ADD COLUMN title VARCHAR(200) NOT NULL DEFAULT '';
ALTER TABLE questions ADD COLUMN tags TEXT[] NOT NULL DEFAULT '{}';

CREATE INDEX idx_questions_tags ON questions USING GIN(tags);
