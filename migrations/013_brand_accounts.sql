-- 샘플 데이터 정리: 브랜드 계정 분리
--
-- 기존 샘플 게시글 13건은 6개 브랜드의 광고인데 전부 관리자(admin@zet.kr) 계정이
-- 작성자로 되어 있어 피드에서 모든 광고의 작성자가 "관리자"로 보였다.
-- 브랜드별 기업 계정(role='business')을 만들고, 관리자가 작성자인 게시글을
-- 본문에 등장하는 브랜드명 기준으로 해당 브랜드 계정에 재배정한다.
-- 샘플 가격(price)도 함께 채운다. 여러 번 실행해도 안전함(ON CONFLICT / 조건부 UPDATE).
--
-- 운영 DB에도 적용될 수 있으므로 브랜드 계정은 로그인 불가 상태로 만든다
-- (password_hash '!login-disabled' 는 argon2 해시가 아니므로 verify_password 가 항상 false).
-- 로컬에서 브랜드로 로그인해 보려면 seeds/dev_sample_activity.sql 을 실행하면
-- 비밀번호가 brand1234 로 설정된다.

INSERT INTO users (email, password_hash, username, name, bio, region, role) VALUES
    ('skinlab@brand.zet.kr',    '!login-disabled', 'skinlab',    '스킨랩 코리아', '피부 고민에 답하는 데일리 스킨케어 브랜드',      '서울', 'business'),
    ('technova@brand.zet.kr',   '!login-disabled', 'technova',   '테크노바',      '가성비를 넘어 가심비까지, 일상 속 테크 기어',     '서울', 'business'),
    ('freshfarm@brand.zet.kr',  '!login-disabled', 'freshfarm',  '프레시팜',      '자연에서 온 건강한 한 끼',                      '서울', 'business'),
    ('fitwear@brand.zet.kr',    '!login-disabled', 'fitwear',    '핏웨어',        '매일 입는 옷, 매일 달리는 신발',                '서울', 'business'),
    ('homeliving@brand.zet.kr', '!login-disabled', 'homeliving', '홈리빙',        '작은 집을 더 쾌적하게',                         '서울', 'business'),
    ('vitalfit@brand.zet.kr',   '!login-disabled', 'vitalfit',   '바이탈핏',      '하루 한 알, 습관이 되는 건강',                  '서울', 'business')
ON CONFLICT (email) DO NOTHING;

-- 관리자 명의 게시글을 본문 브랜드명 기준으로 브랜드 계정에 재배정
UPDATE posts p
SET author_id = u.id
FROM (VALUES
    ('skinlab@brand.zet.kr',    '%스킨랩%'),
    ('technova@brand.zet.kr',   '%테크노바%'),
    ('freshfarm@brand.zet.kr',  '%프레시팜%'),
    ('fitwear@brand.zet.kr',    '%핏웨어%'),
    ('homeliving@brand.zet.kr', '%홈리빙%'),
    ('vitalfit@brand.zet.kr',   '%바이탈핏%')
) AS m(email, pattern)
JOIN users u ON u.email = m.email
WHERE p.author_id = '11111111-1111-1111-1111-111111111111'
  AND p.content LIKE m.pattern;

-- 샘플 가격 (KRW). 이미 값이 있으면 건드리지 않음.
UPDATE posts p
SET price = v.price
FROM (VALUES
    ('bbbb0012-0000-0000-0000-000000000012'::uuid,  39900),
    ('bbbb0006-0000-0000-0000-000000000006'::uuid,  59000),
    ('bbbb0003-0000-0000-0000-000000000003'::uuid,  24900),
    ('bbbb0010-0000-0000-0000-000000000010'::uuid,  89000),
    ('bbbb0005-0000-0000-0000-000000000005'::uuid,  15900),
    ('bbbb0014-0000-0000-0000-000000000014'::uuid,  29800),
    ('bbbb0002-0000-0000-0000-000000000002'::uuid,  18900),
    ('bbbb0007-0000-0000-0000-000000000007'::uuid,  12900),
    ('bbbb0013-0000-0000-0000-000000000013'::uuid,  32000),
    ('bbbb0011-0000-0000-0000-000000000011'::uuid,  79000),
    ('bbbb0009-0000-0000-0000-000000000009'::uuid,  19900),
    ('bbbb0004-0000-0000-0000-000000000004'::uuid, 129000),
    ('bbbb0008-0000-0000-0000-000000000008'::uuid,  34900)
) AS v(id, price)
WHERE p.id = v.id
  AND p.price IS NULL;
