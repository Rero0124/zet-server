-- ============================================================================
-- 개발/데모용 샘플 활동 데이터 (마이그레이션 아님 — 운영 DB에 실행하지 마세요)
--
-- 샘플 게시글(id가 'bbbb'로 시작)과 샘플 사용자(*@test.kr)에만 적용됩니다.
-- 실행할 때마다 날짜를 "지금" 기준으로 다시 배치하므로, 시간이 지나 트렌드/최신 피드가
-- 비어 보이면 다시 실행하면 됩니다.
--
--   psql -d zet -v ON_ERROR_STOP=1 -f seeds/dev_sample_activity.sql
--
-- 하는 일
--   1. 연령/성별/지역이 다양한 샘플 사용자 3명 추가 (트렌드 인구통계 필터용),
--      브랜드 샘플 계정에 로컬 로그인용 비밀번호(brand1234) 부여
--   2. 성의 없는 테스트 리뷰("test") 내용 정리
--   3. 샘플 좋아요/저장/리뷰 추가
--   4. 게시글·반응·노출 기록 날짜를 최근 3주 안으로 재배치
--   5. 샘플 Q&A(브랜드 공식 답변 포함) 추가
--   6. 집계 컬럼(좋아요/리뷰/저장/노출/클릭 수) 재계산
-- ============================================================================

BEGIN;

-- 1) 샘플 사용자 (비밀번호: test1234 — 기존 샘플과 같은 레거시 형식, 로그인 시 argon2로 자동 전환)
INSERT INTO users (id, email, password_hash, username, name, birth_date, gender, region, role) VALUES
  ('66666666-6666-6666-6666-666666666666', 'user5@test.kr', 'hash:test1234', 'user5_6666', '정하은', '1990-04-12', '여성', '서울', 'user'),
  ('77777777-7777-7777-7777-777777777777', 'user6@test.kr', 'hash:test1234', 'user6_7777', '오준혁', '1981-09-03', '남성', '경기', 'user'),
  ('88888888-8888-8888-8888-888888888888', 'user7@test.kr', 'hash:test1234', 'user7_8888', '한지민', '1984-12-20', '여성', '대구', 'user')
ON CONFLICT (email) DO NOTHING;

-- 브랜드 샘플 계정(013에서 로그인 불가로 생성)에 로컬 테스트용 비밀번호 부여: brand1234
UPDATE users SET password_hash = 'hash:brand1234'
WHERE email LIKE '%@brand.zet.kr' AND password_hash = '!login-disabled';

-- 2) 테스트용으로 남은 리뷰 내용 정리
UPDATE reactions
SET content = '노이즈캔슬링이 기대 이상이에요. 지하철에서도 음악에만 집중돼요'
WHERE reaction_type = 'review' AND trim(content) = 'test'
  AND post_id = 'bbbb0004-0000-0000-0000-000000000004';

-- 3) 샘플 반응 (작성자 본인 게시글 제외 — 모두 일반 사용자)
INSERT INTO reactions (post_id, user_id, reaction_type, rating, content)
SELECT v.post_id::uuid, v.user_id::uuid, v.reaction_type, v.rating, v.content
FROM (VALUES
  -- 좋아요
  ('bbbb0002-0000-0000-0000-000000000002', '33333333-3333-3333-3333-333333333333', 'like', NULL::smallint, NULL),
  ('bbbb0007-0000-0000-0000-000000000007', '33333333-3333-3333-3333-333333333333', 'like', NULL, NULL),
  ('bbbb0009-0000-0000-0000-000000000009', '33333333-3333-3333-3333-333333333333', 'like', NULL, NULL),
  ('bbbb0006-0000-0000-0000-000000000006', '44444444-4444-4444-4444-444444444444', 'like', NULL, NULL),
  ('bbbb0005-0000-0000-0000-000000000005', '44444444-4444-4444-4444-444444444444', 'like', NULL, NULL),
  ('bbbb0013-0000-0000-0000-000000000013', '44444444-4444-4444-4444-444444444444', 'like', NULL, NULL),
  ('bbbb0011-0000-0000-0000-000000000011', '44444444-4444-4444-4444-444444444444', 'like', NULL, NULL),
  ('bbbb0009-0000-0000-0000-000000000009', '55555555-5555-5555-5555-555555555555', 'like', NULL, NULL),
  ('bbbb0002-0000-0000-0000-000000000002', '55555555-5555-5555-5555-555555555555', 'like', NULL, NULL),
  ('bbbb0002-0000-0000-0000-000000000002', '66666666-6666-6666-6666-666666666666', 'like', NULL, NULL),
  ('bbbb0013-0000-0000-0000-000000000013', '66666666-6666-6666-6666-666666666666', 'like', NULL, NULL),
  ('bbbb0012-0000-0000-0000-000000000012', '66666666-6666-6666-6666-666666666666', 'like', NULL, NULL),
  ('bbbb0007-0000-0000-0000-000000000007', '66666666-6666-6666-6666-666666666666', 'like', NULL, NULL),
  ('bbbb0014-0000-0000-0000-000000000014', '77777777-7777-7777-7777-777777777777', 'like', NULL, NULL),
  ('bbbb0006-0000-0000-0000-000000000006', '77777777-7777-7777-7777-777777777777', 'like', NULL, NULL),
  ('bbbb0011-0000-0000-0000-000000000011', '77777777-7777-7777-7777-777777777777', 'like', NULL, NULL),
  ('bbbb0014-0000-0000-0000-000000000014', '88888888-8888-8888-8888-888888888888', 'like', NULL, NULL),
  ('bbbb0013-0000-0000-0000-000000000013', '88888888-8888-8888-8888-888888888888', 'like', NULL, NULL),
  ('bbbb0007-0000-0000-0000-000000000007', '88888888-8888-8888-8888-888888888888', 'like', NULL, NULL),
  ('bbbb0010-0000-0000-0000-000000000010', '22222222-2222-2222-2222-222222222222', 'like', NULL, NULL),
  ('bbbb0003-0000-0000-0000-000000000003', '22222222-2222-2222-2222-222222222222', 'like', NULL, NULL),
  -- 저장
  ('bbbb0012-0000-0000-0000-000000000012', '66666666-6666-6666-6666-666666666666', 'bookmark', NULL, NULL),
  ('bbbb0014-0000-0000-0000-000000000014', '77777777-7777-7777-7777-777777777777', 'bookmark', NULL, NULL),
  ('bbbb0002-0000-0000-0000-000000000002', '33333333-3333-3333-3333-333333333333', 'bookmark', NULL, NULL),
  ('bbbb0005-0000-0000-0000-000000000005', '44444444-4444-4444-4444-444444444444', 'bookmark', NULL, NULL),
  ('bbbb0013-0000-0000-0000-000000000013', '88888888-8888-8888-8888-888888888888', 'bookmark', NULL, NULL),
  -- 리뷰
  ('bbbb0002-0000-0000-0000-000000000002', '33333333-3333-3333-3333-333333333333', 'review', 5, '백탁이 하나도 없고 화장 전에 발라도 밀리지 않아요'),
  ('bbbb0002-0000-0000-0000-000000000002', '66666666-6666-6666-6666-666666666666', 'review', 4, '톤업이 자연스러워요. 건성이라 보습은 조금 아쉬워요'),
  ('bbbb0002-0000-0000-0000-000000000002', '55555555-5555-5555-5555-555555555555', 'review', 4, '학생이 쓰기에 가격 부담이 적어서 좋아요'),
  ('bbbb0006-0000-0000-0000-000000000006', '44444444-4444-4444-4444-444444444444', 'review', 5, '설치 20분이면 끝. 책상이 확 넓어졌어요'),
  ('bbbb0014-0000-0000-0000-000000000014', '77777777-7777-7777-7777-777777777777', 'review', 4, '한 달째 먹는 중인데 오후에 눈 피로가 덜해요'),
  ('bbbb0013-0000-0000-0000-000000000013', '88888888-8888-8888-8888-888888888888', 'review', 5, '알약 크기가 작아서 삼키기 편해요'),
  ('bbbb0010-0000-0000-0000-000000000010', '22222222-2222-2222-2222-222222222222', 'review', 5, '쿠셔닝이 좋아서 10km 뛰어도 무릎이 편해요'),
  ('bbbb0012-0000-0000-0000-000000000012', '66666666-6666-6666-6666-666666666666', 'review', 4, '원룸에서는 충분해요. 물통 비우는 게 조금 귀찮아요')
) AS v(post_id, user_id, reaction_type, rating, content)
WHERE EXISTS (SELECT 1 FROM posts WHERE id = v.post_id::uuid)
ON CONFLICT (post_id, user_id, reaction_type) DO NOTHING;

-- 샘플 사용자의 노출/클릭/체류 기록 (사용자×게시글 조합의 약 2/3)
INSERT INTO interactions (post_id, user_id, interaction_type, duration_ms)
SELECT p.id, u.id, t.kind,
       CASE WHEN t.kind = 'dwell' THEN 1500 + abs(hashtext(u.id::text || p.id::text)) % 18000 END
FROM posts p
CROSS JOIN users u
CROSS JOIN (VALUES ('impression'), ('dwell'), ('click')) AS t(kind)
WHERE p.id::text LIKE 'bbbb%'
  AND u.email LIKE '%@test.kr'
  AND abs(hashtext(u.id::text || p.id::text)) % 3 <> 0
  AND (t.kind <> 'click' OR abs(hashtext(p.id::text || u.id::text)) % 3 = 0)
  AND NOT EXISTS (
    SELECT 1 FROM interactions i
    WHERE i.post_id = p.id AND i.user_id = u.id AND i.interaction_type = t.kind
  );

-- 4) 날짜 재배치: 게시글은 최근 약 3주에 고르게, 반응·기록은 각 게시 시점부터 지금 사이에 분산
WITH ordered AS (
  SELECT id,
         row_number() OVER (ORDER BY created_at, id) AS rn,
         count(*) OVER () AS total
  FROM posts
  WHERE id::text LIKE 'bbbb%'
)
UPDATE posts p
SET created_at = now() - ((o.total - o.rn) * interval '40 hours') - interval '20 hours',
    updated_at = now() - ((o.total - o.rn) * interval '40 hours') - interval '20 hours'
FROM ordered o
WHERE p.id = o.id;

UPDATE reactions r
SET created_at = p.created_at + interval '1 hour'
      + (now() - p.created_at - interval '2 hours') * ((abs(hashtext(r.id::text)) % 1000) / 1000.0)
FROM posts p
WHERE p.id = r.post_id AND p.id::text LIKE 'bbbb%';

UPDATE interactions i
SET created_at = p.created_at + interval '30 minutes'
      + (now() - p.created_at - interval '1 hour') * ((abs(hashtext(i.id::text)) % 1000) / 1000.0)
FROM posts p
WHERE p.id = i.post_id AND p.id::text LIKE 'bbbb%';

-- 5) 샘플 Q&A
INSERT INTO questions (post_id, user_id, title, content, tags, created_at, updated_at)
SELECT v.post_id::uuid, v.user_id::uuid, v.title, v.content, v.tags,
       now() - v.ago, now() - v.ago
FROM (VALUES
  ('bbbb0002-0000-0000-0000-000000000002', '33333333-3333-3333-3333-333333333333',
   '민감성 피부도 써도 되나요?', '트러블이 잘 나는 편인데 자극 없이 쓸 수 있을까요?', ARRAY['선크림', '민감성'], interval '3 days'),
  ('bbbb0006-0000-0000-0000-000000000006', '44444444-4444-4444-4444-444444444444',
   '책상 두께 몇 cm까지 고정되나요?', '원목 책상이 4.5cm인데 클램프로 고정 가능한지 궁금해요.', ARRAY['모니터암', '설치'], interval '2 days'),
  ('bbbb0010-0000-0000-0000-000000000010', '22222222-2222-2222-2222-222222222222',
   '평발인데 사이즈 반 업 해야 할까요?', '평소 270 신는데 발볼이 넓은 편이에요.', ARRAY['러닝화', '사이즈'], interval '30 hours'),
  ('bbbb0013-0000-0000-0000-000000000013', '88888888-8888-8888-8888-888888888888',
   '다른 영양제와 같이 먹어도 되나요?', '오메가3를 함께 먹고 있는데 같이 먹어도 괜찮을까요?', ARRAY['비타민', '복용법'], interval '6 hours')
) AS v(post_id, user_id, title, content, tags, ago)
WHERE EXISTS (SELECT 1 FROM posts WHERE id = v.post_id::uuid)
  AND NOT EXISTS (
    SELECT 1 FROM questions q WHERE q.post_id = v.post_id::uuid AND q.user_id = v.user_id::uuid AND q.title = v.title
  );

-- 답변: 브랜드 공식 답변 + 사용자 답변 (마지막 질문은 '답변 대기' 상태로 둠)
INSERT INTO answers (question_id, user_id, content, created_at, updated_at)
SELECT q.id, a.user_id, a.content, q.created_at + a.after, q.created_at + a.after
FROM (VALUES
  ('민감성 피부도 써도 되나요?', (SELECT p.author_id FROM posts p WHERE p.id = 'bbbb0002-0000-0000-0000-000000000002'),
   '안녕하세요, 스킨랩입니다. 피부 자극 테스트를 완료한 제품이라 민감성 피부도 사용하실 수 있어요. 처음 쓰실 땐 팔 안쪽에 패치 테스트를 권해드려요.', interval '5 hours'),
  ('민감성 피부도 써도 되나요?', '66666666-6666-6666-6666-666666666666'::uuid,
   '저도 민감성인데 한 달째 트러블 없이 쓰고 있어요!', interval '20 hours'),
  ('책상 두께 몇 cm까지 고정되나요?', (SELECT p.author_id FROM posts p WHERE p.id = 'bbbb0006-0000-0000-0000-000000000006'),
   '테크노바입니다. 클램프 기준 1~9cm 두께까지 고정되니 4.5cm 원목 책상에도 문제없이 설치하실 수 있어요.', interval '3 hours'),
  ('평발인데 사이즈 반 업 해야 할까요?', (SELECT p.author_id FROM posts p WHERE p.id = 'bbbb0010-0000-0000-0000-000000000010'),
   '핏웨어입니다. 발볼이 넓으시면 반 사이즈 업(275)을 추천드려요. 교환은 1회 무료예요.', interval '2 hours')
) AS a(title, user_id, content, after)
JOIN questions q ON q.title = a.title
WHERE a.user_id IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM answers x WHERE x.question_id = q.id AND x.user_id = a.user_id);

-- 6) 집계 컬럼 재계산
UPDATE posts p SET
  like_count     = (SELECT count(*) FROM reactions r WHERE r.post_id = p.id AND r.reaction_type = 'like'),
  review_count   = (SELECT count(*) FROM reactions r WHERE r.post_id = p.id AND r.reaction_type = 'review'),
  bookmark_count = (SELECT count(*) FROM reactions r WHERE r.post_id = p.id AND r.reaction_type = 'bookmark'),
  impressions    = (SELECT count(DISTINCT i.user_id) FROM interactions i WHERE i.post_id = p.id AND i.interaction_type = 'impression'),
  clicks         = (SELECT count(DISTINCT i.user_id) FROM interactions i WHERE i.post_id = p.id AND i.interaction_type = 'click')
WHERE p.id::text LIKE 'bbbb%';

UPDATE questions q SET answer_count = (SELECT count(*) FROM answers a WHERE a.question_id = q.id);

COMMIT;
