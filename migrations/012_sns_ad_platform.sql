-- SNS 광고 플랫폼 확장: 광고 링크/CTA/가격, 프로필 소개/아바타, 팔로우, 조회 인덱스
-- 모든 구문은 IF NOT EXISTS 로 멱등하게 작성됨.

-- 게시글: 외부 링크, CTA 버튼 문구, 가격(KRW)
ALTER TABLE posts ADD COLUMN IF NOT EXISTS link_url TEXT;
ALTER TABLE posts ADD COLUMN IF NOT EXISTS cta_label VARCHAR(20);
ALTER TABLE posts ADD COLUMN IF NOT EXISTS price BIGINT;

-- 사용자: 소개, 프로필 이미지
ALTER TABLE users ADD COLUMN IF NOT EXISTS bio VARCHAR(300);
ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_url TEXT;

-- 팔로우 (follower → followee)
CREATE TABLE IF NOT EXISTS follows (
    follower_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    followee_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (follower_id, followee_id),
    CHECK (follower_id <> followee_id)
);
CREATE INDEX IF NOT EXISTS idx_follows_followee ON follows(followee_id);

-- 조회 최적화 인덱스 (저장한 글 목록, 성과 집계)
CREATE INDEX IF NOT EXISTS idx_reactions_user_type_created ON reactions(user_id, reaction_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_interactions_post_type ON interactions(post_id, interaction_type);
