# zet-server

Zet(제품 트렌드) API 서버. Rust + Axum + PostgreSQL.

## 요구사항

- Rust (edition 2024)
- PostgreSQL

## 설정

```bash
# DB 생성
psql -d postgres -c "CREATE USER zet WITH PASSWORD 'zet1234';"
psql -d postgres -c "CREATE DATABASE zet OWNER zet;"
psql -d postgres -c "GRANT ALL PRIVILEGES ON DATABASE zet TO zet;"

# 마이그레이션 적용
psql -d zet -f migrations/001_initial.sql
psql -d zet -f migrations/002_search_and_trends.sql
psql -d zet -f migrations/003_birthdate_and_posts.sql
psql -d zet -f migrations/004_post_author.sql
psql -d zet -f migrations/005_interactions.sql

# 권한 부여 (테이블 owner가 다른 경우)
psql -d zet -c "GRANT ALL ON ALL TABLES IN SCHEMA public TO zet;"
psql -d zet -c "GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO zet;"
```

환경변수 (`.env`):
```
DATABASE_URL=postgres://zet:zet1234@localhost/zet
```

## 실행

```bash
cargo run  # http://localhost:3002
```

포트는 `PORT` 환경변수로 변경 가능 (기본 3002).

## 프로젝트 구조

```
src/
├── main.rs          # 서버 진입점, CORS, 라우터 조합
├── db.rs            # type Db = PgPool
├── models/
│   ├── user.rs      # User, CreateUser, LoginRequest
│   ├── post.rs      # Post, CreatePost
│   └── reaction.rs  # Reaction, CreateReaction
└── routes/
    ├── auth.rs          # 회원가입/로그인, 기업 회원 시 companies 자동 등록
    ├── posts.rs         # 게시글 CRUD + 유저별 게시글 목록
    ├── reactions.rs     # 리뷰/좋아요/북마크 CRUD
    ├── feed.rs          # 통합 피드 (추천/최신/인기, 가중 스코어링)
    ├── search.rs        # 전문 검색 (tsvector)
    ├── trending.rs      # 트렌드 (실제 사용자 반응 기반 인구통계별 집계)
    ├── profile.rs       # 프로필 조회/수정
    └── interactions.rs  # 인터랙션 기록 (impression/dwell/click)
```

## API

### 인증
| Method | Path | 설명 |
|--------|------|------|
| POST | `/api/auth/register` | 회원가입 (기업 여부 + 기업 정보 포함) |
| POST | `/api/auth/login` | 로그인 |

### 사용자
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/users/{id}` | 프로필 조회 |
| PUT | `/api/users/{id}` | 프로필 수정 |
| GET | `/api/users/{id}/posts` | 유저의 게시글 목록 |

### 게시글
| Method | Path | 설명 |
|--------|------|------|
| POST | `/api/posts` | 작성 (기업 회원만) |
| GET | `/api/posts/{id}` | 상세 |
| PUT | `/api/posts/{id}` | 수정 (본인만) |
| DELETE | `/api/posts/{id}?author_id=` | 삭제 (본인만, CASCADE) |

### 반응
| Method | Path | 설명 |
|--------|------|------|
| POST | `/api/posts/{id}/reactions` | 리뷰 작성 |
| GET | `/api/posts/{id}/reactions` | 리뷰 목록 |
| PUT | `/api/posts/{post_id}/reactions/{id}` | 리뷰 수정 (본인만) |
| DELETE | `/api/posts/{post_id}/reactions/{id}?user_id=` | 리뷰 삭제 (본인만) |
| POST | `/api/posts/{id}/like` | 좋아요 토글 |
| POST | `/api/posts/{id}/bookmark` | 북마크 토글 |

### 피드/검색/트렌드
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/feed?sort=&user_id=&category=` | 통합 피드 |
| GET | `/api/search?q=&category=` | 전문 검색 |
| GET | `/api/trending?period=&age_group=&gender=&region=` | 트렌드 게시글 |
| GET | `/api/trending/keywords?...` | 인기 키워드 |

### 인터랙션
| Method | Path | 설명 |
|--------|------|------|
| POST | `/api/interactions` | 단건 기록 |
| POST | `/api/interactions/batch` | 배치 기록 |

## 추천 알고리즘

피드는 단일 스코어링 함수로 정렬됩니다:

```
총점 = W1 × 인구매칭 + W2 × 최신도 + W3 × 인기도 + W4 × 카테고리선호도
```

| 정렬 | 인구매칭(W1) | 최신도(W2) | 인기도(W3) | 선호도(W4) |
|------|------------|-----------|-----------|-----------|
| 추천순 | 3 | 1 | 1 | 3 |
| 최신순 | 1 | 5 | 1 | 1 |
| 인기순 | 1 | 1 | 5 | 1 |

- **인구매칭**: 나이대 +3, 성별 +2, 지역 +2 (로그인 유저 기준)
- **최신도**: 0~10 (7일 선형 감소)
- **인기도**: score + ln(좋아요) + ln(리뷰수)
- **카테고리선호도**: 과거 인터랙션/반응 횟수의 로그값

## DB 스키마

테이블: `users`, `companies`, `posts`, `reactions`, `impressions`, `clicks`, `interactions`

주요 특징:
- `users.birth_date` → `age_group_from_birth()` DB 함수로 나이대 계산
- `posts.search_vector` (tsvector + GIN) + 자동 업데이트 트리거
- `posts.like_count`, `review_count`, `bookmark_count` 집계 컬럼 (반응 시 자동 갱신)
- `interactions`: impression/dwell/click 기록 (duration_ms 포함)
- 트렌드는 게시글 타겟이 아닌 **실제 사용자 반응** 기반
