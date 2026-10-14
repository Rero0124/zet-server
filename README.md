# zet-server

Zet(광고 전용 SNS) API 서버. Rust + Axum + PostgreSQL.

브랜드(기업 회원)가 제품 광고를 게시하고, 사용자는 좋아요/저장/리뷰/Q&A/팔로우로 반응합니다.

## 요구사항

- Rust (edition 2024)
- PostgreSQL

## 설정

```bash
# DB 생성
psql -d postgres -c "CREATE USER zet WITH PASSWORD 'zet1234';"
psql -d postgres -c "CREATE DATABASE zet OWNER zet;"
psql -d postgres -c "GRANT ALL PRIVILEGES ON DATABASE zet TO zet;"

# 마이그레이션 적용 (순서대로, 각 1회)
psql -d zet -v ON_ERROR_STOP=1 -f migrations/001_initial.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/002_search_and_trends.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/003_birthdate_and_posts.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/004_post_author.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/005_interactions.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/006_reaction_media.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/007_post_blocks.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/008_drop_registration_no.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/009_username.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/010_qna.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/011_question_title_tags.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/012_sns_ad_platform.sql
psql -d zet -v ON_ERROR_STOP=1 -f migrations/013_brand_accounts.sql

# 권한 부여 (테이블 owner가 다른 경우)
psql -d zet -c "GRANT ALL ON ALL TABLES IN SCHEMA public TO zet;"
psql -d zet -c "GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO zet;"
```

| 마이그레이션 | 내용 |
|---|---|
| 001 | users / companies / posts / reactions / impressions / clicks |
| 002 | 전문 검색(tsvector + GIN), 반응 집계 컬럼 |
| 003 | birth_date 도입, `age_group_from_birth()` |
| 004 | posts.author_id (일반 유저 작성 허용) |
| 005 | interactions 통합 테이블 |
| 006 | reactions.media_urls |
| 007 | posts.blocks (블록 기반 본문) |
| 008 | companies 테이블 / posts.company_id 제거 |
| 009 | users.username |
| 010 | questions / answers (Q&A) |
| 011 | questions.title / tags |
| 012 | posts.link_url / cta_label / price, users.bio / avatar_url, follows 테이블, 조회 인덱스 |
| 013 | 샘플 데이터 정리: 브랜드 계정 6개 생성 후 관리자 명의 샘플 게시글 재배정, 샘플 가격 |

008~011 은 멱등하지 않으므로 정확히 한 번만 적용합니다. 012, 013 은 여러 번 실행해도 안전합니다.

### (선택) 개발용 샘플 활동 데이터

```bash
psql -d zet -v ON_ERROR_STOP=1 -f seeds/dev_sample_activity.sql
```

마이그레이션이 아닌 **로컬/데모 전용** 스크립트입니다. 샘플 사용자 3명(user5~7@test.kr / test1234)과 좋아요·저장·리뷰·노출 기록, 브랜드 답변이 달린 Q&A를 추가하고, 샘플 게시글(`bbbb…`)과 반응 날짜를 최근 3주 안으로 재배치합니다. 시간이 지나 트렌드가 비어 보이면 다시 실행하세요. 운영 DB에는 실행하지 마세요.

환경변수 (`.env`):
```
DATABASE_URL=postgres://zet:zet1234@localhost/zet
```

## 실행

```bash
cargo run  # http://localhost:3002
```

포트는 `PORT` 환경변수로 변경 가능 (기본 3002).
S3 없이 로컬 저장소만 쓰려면 `cargo run --no-default-features --features local-storage`.

## 프로젝트 구조

```
src/
├── main.rs          # 서버 진입점, CORS, 라우터 조합
├── db.rs            # type Db = PgPool
├── util.rs          # 공용 헬퍼: deserialize_some(생략/null 구분), LIKE 이스케이프, tsquery 정제, UNIQUE 위반→409
├── storage.rs       # 업로드 저장소 (S3 / 로컬)
├── models/
│   ├── user.rs      # User(bio, avatar_url 포함), CreateUser, LoginRequest
│   ├── post.rs      # Post, PostView(+작성자/평점/질문수/liked/bookmarked), post_view_select(), CreatePost
│   ├── reaction.rs  # Reaction, ReactionView(+작성자), CreateReaction
│   └── question.rs  # Question, Answer
└── routes/
    ├── auth.rs          # 회원가입/로그인 (중복 이메일·아이디는 409)
    ├── posts.rs         # 게시글 CRUD, 유저별 게시글, 저장한 게시글
    ├── reactions.rs     # 리뷰/좋아요/북마크 CRUD, 유저별 리뷰 목록
    ├── feed.rs          # 통합 피드 (추천/최신/인기 + 팔로잉 필터, 가중 스코어링)
    ├── search.rs        # 검색 (prefix tsquery + 본문/태그/작성자 부분 일치)
    ├── trending.rs      # 트렌드 키워드
    ├── profile.rs       # 프로필 조회/수정, 유저 검색, 팔로우 토글
    ├── brands.rs        # 브랜드(기업 계정) 목록
    ├── insights.rs      # 게시글 성과 (작성자 전용)
    ├── interactions.rs  # 인터랙션 기록 (impression/dwell/click/cta)
    ├── qna.rs           # 질문/답변
    ├── upload.rs        # 파일 업로드
    └── ai.rs            # AI 전용 엔드포인트 (/ai/*, /api 밖 별도 마운트)
```

## API

모든 경로는 `/api` 아래에 있습니다. 에러는 `{"error": "메시지"}` 형태입니다.
목록 응답의 `next_cursor` 는 다음 페이지의 `cursor` 값(offset)입니다.

### 인증
| Method | Path | 설명 |
|--------|------|------|
| POST | `/auth/register` | 회원가입 (`is_business` 로 기업 여부). 중복 이메일/아이디는 409 |
| POST | `/auth/login` | 로그인 → `{ user }` |

### 사용자 / 프로필
| Method | Path | 설명 |
|--------|------|------|
| GET | `/users/{id}?viewer_id=` | `{ user, stats: { posts, reviews, followers, following }, is_following }` |
| GET | `/users/by-username/{username}?viewer_id=` | 위와 동일, 아이디로 조회 |
| PUT | `/users/{id}` | 프로필 수정. `bio`, `avatar_url` 은 생략 → 유지, null/빈 문자열 → 삭제. 아이디 중복은 409 |
| GET | `/users/search?q=&limit=` | 유저 검색 (아이디 접두 일치, 이름 부분 일치, 기업 계정 우선). `id, username, name, role, avatar_url, bio` |
| POST | `/users/{id}/follow` | 팔로우 토글. body `{ "user_id": "<팔로우하는 쪽>" }` → `{ following, follower_count }` |
| GET | `/users/{id}/posts?viewer_id=&cursor=&limit=` | 유저의 게시글 (`PostView[]`, 최신순) |
| GET | `/users/{id}/bookmarks?viewer_id=&cursor=&limit=` | 저장한 게시글 (본인만, 저장 시각 역순) |
| GET | `/users/{id}/reviews?cursor=&limit=` | 유저가 남긴 리뷰 (`ReactionView` + `post_content`, `post_category`, `post_author_name`, `post_author_username`) |

### 브랜드
| Method | Path | 설명 |
|--------|------|------|
| GET | `/brands?viewer_id=&limit=` | 활성 게시글이 있는 기업 계정 (기본 10, 최대 30). `id, username, name, bio, avatar_url, post_count, follower_count, total_likes, is_following` |

### 게시글
| Method | Path | 설명 |
|--------|------|------|
| POST | `/posts` | 작성 (기업 회원만). `link_url`(http(s) 필수), `cta_label`(20자), `price`(0 이상) 포함 |
| GET | `/posts/{id}?viewer_id=` | 상세 → `{ post: PostView }` |
| PUT | `/posts/{id}` | 수정 (본인만). `link_url`, `cta_label`, `price`, `target_age`, `target_gender`, `target_region` 은 생략 → 유지, null/빈 문자열 → 삭제 |
| DELETE | `/posts/{id}?author_id=` | 삭제 (본인만, CASCADE) |
| GET | `/posts/{id}/insights?author_id=` | 성과 (본인만). 노출/클릭/CTA/CTR/평균 체류/반응 수/평점, 최근 14일 일별 추이, 오디언스 분포 |

`PostView` = `Post` 전체 + `author_name`, `author_username`, `author_role`, `author_avatar_url`, `avg_rating`, `question_count`, `liked`, `bookmarked` (뷰어 기준).

### 반응
| Method | Path | 설명 |
|--------|------|------|
| POST | `/posts/{id}/reactions` | 반응 작성 → `{ reaction: ReactionView }`. 리뷰는 별점 1~5 와 내용 필수, 본인 게시글에는 불가(403) |
| GET | `/posts/{id}/reactions` | 반응 목록 (`ReactionView[]`) |
| PUT | `/posts/{post_id}/reactions/{id}` | 수정 (본인만). `content`, `rating`, `media_urls` |
| DELETE | `/posts/{post_id}/reactions/{id}?user_id=` | 삭제 (본인만) |
| POST | `/posts/{id}/like` | 좋아요 토글 |
| POST | `/posts/{id}/bookmark` | 북마크 토글 |
| GET | `/posts/{id}/status?user_id=` | 좋아요/북마크 여부 |

`ReactionView` = `Reaction` 전체 + `user_name`, `user_username`, `user_avatar_url`.

### 피드 / 검색 / 트렌드
| Method | Path | 설명 |
|--------|------|------|
| GET | `/feed?sort=&user_id=&category=&following=&cursor=&limit=` | 통합 피드 (`PostView[]`). `sort` = recommended(기본) / popular / latest. `following=true` 면 팔로우한 작성자만 |
| GET | `/search?q=&category=&viewer_id=&cursor=&limit=` | 검색 (`PostView[]`). 앞의 `#` 은 무시, 특수문자만 있는 입력도 안전 |
| GET | `/trending/keywords?period=&age_group=&gender=&region=` | 인기 키워드 |
| GET | `/trending/keywords/{keyword}` | 키워드 상세 |

### Q&A
| Method | Path | 설명 |
|--------|------|------|
| GET | `/questions?post_id=&user_id=&tag=&q=&cursor=&limit=` | 질문 목록. 각 항목에 `post_content`, `post_category`, `post_author_id`, `post_author_name`, `post_author_username` 포함 |
| POST | `/questions` | 질문 작성 |
| GET | `/questions/{id}` | 질문 + 답변. 답변에는 `is_brand`(게시글 작성자 여부) 포함 |
| DELETE | `/questions/{id}?user_id=` | 삭제 (본인만) |
| GET | `/questions/{id}/answers` | 답변 목록 (`is_brand` 포함) |
| POST | `/questions/{id}/answers` | 답변 작성 |
| DELETE | `/questions/{qid}/answers/{id}?user_id=` | 답변 삭제 (본인만) |

### 인터랙션
| Method | Path | 설명 |
|--------|------|------|
| POST | `/interactions` | 단건 기록 |
| POST | `/interactions/batch` | 배치 기록 |

`interaction_type` 은 `impression`, `dwell`, `click`, `cta`(광고 링크 클릭) 만 허용하며 그 외는 조용히 무시됩니다.
`impression`/`click`/`cta` 는 유저·게시글·일 단위로 1회만 기록됩니다.

### 업로드
| Method | Path | 설명 |
|--------|------|------|
| POST | `/upload` | multipart 이미지/동영상 업로드 → `{ urls }` |

### AI 전용 (`/ai/*` — `/api` 밖 별도 마운트, 인증 불필요)
| Method | Path | 설명 |
|--------|------|------|
| GET | `/ai/search?q=&limit=` | 게시글 검색 + 리뷰 요약 + 상위 리뷰 |
| GET | `/ai/product/{id}` | 제품 상세 + 전체 리뷰 + 기업 정보 |
| GET | `/ai/trending?period=&category=&limit=` | 인기 제품 + 트렌드 키워드 |
| GET | `/ai/category/{name}` | 카테고리별 인기 제품 + 리뷰 요약 |

AI 응답은 한 요청에 게시글 + 리뷰 + 통계를 풍부하게 담아 반환.
`llms.txt` (웹 public/)에 AI용 API 문서 제공.

## 추천 알고리즘

피드는 단일 스코어링 함수로 정렬됩니다:

```
총점 = W1 × 인구매칭 + W2 × 최신도 + W3 × 인기도 + W4 × 카테고리선호도 + W5 × 팔로우
```

| 정렬 | 인구매칭(W1) | 최신도(W2) | 인기도(W3) | 선호도(W4) | 팔로우(W5) |
|------|------------|-----------|-----------|-----------|-----------|
| 추천순 | 3 | 1 | 1 | 3 | 2 |
| 최신순 | 1 | 5 | 1 | 1 | 0.5 |
| 인기순 | 1 | 1 | 5 | 1 | 0.5 |

- **인구매칭**: 나이대 +3, 성별 +2, 지역 +2 (로그인 유저 기준)
- **최신도**: 0~10 (7일 선형 감소)
- **인기도**: score + ln(좋아요) + ln(리뷰수)
- **카테고리선호도**: 과거 인터랙션/반응 횟수의 로그값
- **팔로우**: 뷰어가 작성자를 팔로우 중이면 +3

## DB 스키마

테이블: `users`, `posts`, `reactions`, `follows`, `questions`, `answers`, `interactions`, `impressions`, `clicks`

주요 특징:
- `users.birth_date` → `age_group_from_birth()` DB 함수로 나이대 계산
- `users.bio`, `avatar_url` 프로필 소개/이미지
- `posts.link_url`, `cta_label`, `price` 광고 링크/버튼 문구/가격(KRW)
- `posts.search_vector` (tsvector + GIN) + 자동 업데이트 트리거
- `posts.like_count`, `review_count`, `bookmark_count` 집계 컬럼 (반응 시 자동 갱신)
- `follows (follower_id, followee_id)` 팔로우 관계 (자기 자신 금지)
- `interactions`: impression/dwell/click/cta 기록 (duration_ms 포함)
- 트렌드는 게시글 타겟이 아닌 **실제 사용자 반응** 기반

## 샘플 데이터

시드 계정 비밀번호는 `hash:` 레거시 형식이며 첫 로그인 시 argon2 로 자동 승격됩니다.

| 계정 | 비밀번호 | 역할 |
|---|---|---|
| admin@zet.kr | admin123 | business |
| user1@test.kr ~ user4@test.kr | test1234 | user |
| skinlab / technova / freshfarm / fitwear / homeliving / vitalfit `@brand.zet.kr` | brand1234 (로컬 시드 실행 시) | business (013에서 로그인 불가로 생성, `seeds/dev_sample_activity.sql`이 로컬용 비밀번호 부여) |
