# 만화 뷰어 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 로컬·SFTP 폴더의 ZIP 만화를 표지 책장으로 보여 주고, 일본식/한국식 방향의 두 쪽 펼침으로 읽는 Tauri 2 데스크톱 앱.

**Architecture:** Rust 코어가 "소스(로컬/SFTP) → 블록 캐시 Read+Seek → ZIP 인덱스 → 작품·권 묶기"를 맡고, 페이지·표지 이미지는 `comic://` URI 스킴으로 웹뷰에 직접 넘긴다. 진행·설정·스캔 캐시는 SQLite에 저장한다. React 프론트엔드는 책장(사이드바+그리드)과 읽는 화면을 그리고, 펼침 짝짓기·키 매핑은 순수 TS 함수로 둔다.

**Tech Stack:** Tauri 2.12, Rust (zip 9.x, encoding_rs 0.8, russh 0.65 + russh-sftp 4.0, ssh2-config 0.9, rusqlite 0.43, keyring 4, image 0.25, lru 0.19, tokio 1), React 19 + TypeScript + Vite 8, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-24-comics-viewer-design.md`

## Global Constraints

- 지원 포맷은 ZIP(.zip, .cbz)뿐이다. RAR/PDF/7z/이미지 폴더는 어디에도 넣지 않는다(선택지·문구 포함).
- 원격 서버에는 아무것도 쓰지 않는다. 모든 상태는 로컬 앱 데이터/캐시 폴더에 둔다.
- 기본 넘김 방향은 일본식(`rtl`): `ArrowLeft` = 다음, `ArrowRight` = 이전. 한국식(`ltr`)은 반대. `Space` = 다음, `Shift+Space` = 이전.
- ZIP 이름 디코딩: UTF-8 플래그 → UTF-8, 플래그 없음 → UTF-8 유효하면 UTF-8, 아니면 CP949.
- 정렬은 항상 자연 정렬(숫자 크기 비교).
- 이미지 확장자: `jpg jpeg png gif webp bmp`(대소문자 무시). `__MACOSX/` 하위와 `._`로 시작하는 파일은 제외.
- 라이브러리 API는 구현 전에 context7로 최신 문서를 확인한다(사용자 전역 지침). 아래 버전은 2026-11-24 crates.io/npm 기준.
- SQL 파일의 주석은 영어로 쓴다.
- UI 문구는 한국어, 짧고 정확하게. 당연한 동작 설명·내부 처리 중계 문구를 넣지 않는다.
- 숨는 막대·로딩 표시 때문에 레이아웃 크기가 바뀌거나 깜빡이지 않게 한다(막대는 오버레이).

## Review Focus

1. **macOS에서 만든 ZIP의 `__MACOSX/._001.jpg`** → 가짜 권이 생기지 않아야 한다. (Task 3 테스트)
2. **UTF-8 플래그 없이 UTF-8로 저장된 이름**(맥에서 만든 ZIP) → CP949로 잘못 디코딩하지 않아야 한다. (Task 1 테스트)
3. **깨진 ZIP·이미지 없는 ZIP이 서재에 섞임** → 스캔 전체가 실패하지 않고 그 파일만 빠진다. (Task 4 테스트)
4. **진행 기록이 있는 책이 삭제·이동됨** → 책장과 다음 권 계산이 오류 없이 동작한다. (Task 5 테스트)
5. **SFTP 읽기 중 연결 끊김** → 블록 리더가 오류를 그대로 전달하고(멈추지 않음), 소스가 한 번 재연결 후 재시도한다. (Task 2, Task 10 테스트)

---

## 파일 구조

```
package.json, vite.config.ts, tsconfig.json, index.html
src/
  main.tsx, App.tsx, styles.css
  api.ts                    # invoke 래퍼 + 공용 타입 + comic URL 헬퍼
  shelf/Sidebar.tsx, WorkGrid.tsx, VolumeGrid.tsx, BookCard.tsx, Progress.tsx
  library/AddLibraryDialog.tsx, RemoteBrowser.tsx, SshPromptDialog.tsx
  reader/Reader.tsx, spread.ts, keys.ts, usePrefetch.ts
  reader/spread.test.ts, reader/keys.test.ts
src-tauri/
  Cargo.toml, tauri.conf.json, capabilities/default.json
  src/main.rs, lib.rs        # 빌더, 상태, 커맨드/프로토콜 등록
  src/natsort.rs, encoding.rs, direction.rs
  src/blockcache.rs          # RangeRead → Read+Seek, LRU 블록
  src/source/mod.rs, local.rs
  src/source/sftp/mod.rs, config.rs, session.rs, file.rs, prompt.rs
  src/archive.rs             # ZipIndex
  src/library.rs             # 스캔 + 작품·권 묶기
  src/db.rs, src/schema.sql
  src/thumbs.rs, protocol.rs, commands.rs, state.rs
  tests/fixtures.rs          # 테스트용 ZIP 생성 헬퍼
```

---

### Task 0: 환경과 스캐폴딩

**Files:** Create: 위 구조의 스캐폴드 전체, `.gitignore`(이미 있음: `.superpowers/ node_modules/ dist/ src-tauri/target/`)

- [ ] **Step 1:** Rust 설치: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y`, 이후 `source ~/.cargo/env && rustc --version` 확인.
- [ ] **Step 2:** `bun create tauri-app@4.7.5 . --template react-ts --manager bun --identifier dev.local.comicsviewer -y` (현재 폴더에 기존 `docs/`, `.gitignore`가 있으므로 임시 폴더에 생성 후 옮겨도 됨). 앱 이름 `comics-viewer`, 창 제목 `만화 뷰어`, 기본 창 1280×860.
- [ ] **Step 3:** Rust 의존성 추가(`src-tauri/Cargo.toml`): `zip`, `encoding_rs`, `rusqlite = { features=["bundled"] }`, `image`(jpeg/png/gif/webp/bmp만), `lru`, `serde`, `serde_json`, `thiserror`, `sha1`/`hex`, `tokio`(rt-multi-thread, sync), `tauri-plugin-dialog`. 개발 의존성 `tempfile`. 프론트 `vitest` 추가, `package.json`에 `"test": "vitest run"`.
- [ ] **Step 4:** 실행 확인: `bun run tauri dev` → "만화 뷰어" 창이 뜬다. `cd src-tauri && cargo test` 0개 통과, `bun run test` 통과(빈 스위트 허용 `--passWithNoTests`).
- [ ] **Step 5:** `git init && git add -A && git commit -m "chore: scaffold Tauri 2 + React app"`

### Task 1: 자연 정렬과 ZIP 이름 디코딩

**Files:** Create `src-tauri/src/natsort.rs`, `src-tauri/src/encoding.rs` (테스트는 각 파일의 `#[cfg(test)]`)

**Interfaces:**
- Produces: `natsort::cmp(a: &str, b: &str) -> std::cmp::Ordering`, `encoding::decode_name(raw: &[u8], utf8_flag: bool) -> String`

- [ ] **Step 1: 실패하는 테스트**

```rust
// natsort
assert_eq!(sorted(["생존게임 10.zip","생존게임 2.zip","생존게임 01.zip"]),
           ["생존게임 01.zip","생존게임 2.zip","생존게임 10.zip"]);
assert_eq!(sorted(["킹덤 (KINGDIM)  (530)","킹덤 (KINGDIM)  (526)"])[0], "킹덤 (KINGDIM)  (526)");
assert_eq!(sorted(["생존게임 21(완).zip","생존게임 20.zip"])[0], "생존게임 20.zip");
assert_eq!(cmp("a01","a1"), Ordering::Less); // 같은 값이면 0 패딩 긴 쪽이 앞(안정적 순서)
// encoding
let (cp949, _, _) = encoding_rs::EUC_KR.encode("호문쿨루스 01권/001.jpg");
assert_eq!(decode_name(&cp949, false), "호문쿨루스 01권/001.jpg");
assert_eq!(decode_name("슬램덩크/01.jpg".as_bytes(), false), "슬램덩크/01.jpg"); // 플래그 없는 UTF-8
assert_eq!(decode_name("a.jpg".as_bytes(), true), "a.jpg");
```

- [ ] **Step 2:** `cargo test natsort encoding` → 컴파일 실패(FAIL) 확인.
- [ ] **Step 3:** 구현. natsort는 숫자 구간을 0을 뗀 길이→사전순으로 비교, 숫자 외 구간은 문자 비교(대소문자 무시). decode_name은 `std::str::from_utf8` 성공 시 그대로, 실패 시 `encoding_rs::EUC_KR.decode`(CP949 상위호환).
- [ ] **Step 4:** `cargo test natsort encoding` → PASS.
- [ ] **Step 5:** 커밋 `feat: natural sort and zip name decoding`

### Task 2: 블록 캐시 리더와 로컬 소스

**Files:** Create `src-tauri/src/blockcache.rs`, `src-tauri/src/source/mod.rs`, `src-tauri/src/source/local.rs`

**Interfaces:**
- Produces:
  - `trait RangeRead: Send { fn len(&self) -> u64; fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize>; }`
  - `BlockReader<R: RangeRead>::new(inner: R, block_size: usize, max_blocks: usize)` — `Read + Seek` 구현, `fn bytes_fetched(&self) -> u64`
  - 상수 `BLOCK_SIZE = 256 * 1024`, `MAX_BLOCKS = 64`
  - `trait ReadSeek: Read + Seek + Send {}` (블랭킷 구현)
  - `struct DirEntry { name: String, is_dir: bool, size: u64, mtime: i64 }`
  - `trait Source: Send + Sync { fn list_dir(&self, rel: &str) -> io::Result<Vec<DirEntry>>; fn open(&self, rel: &str) -> io::Result<Box<dyn ReadSeek>>; }` — `rel`은 서재 루트 기준 `/` 구분 경로, 루트는 `""`
  - `LocalSource::new(root: PathBuf)`

- [ ] **Step 1: 실패하는 테스트** (`blockcache.rs`, 호출 횟수를 세는 `MemRange` 테스트 더블 사용)

```rust
// 10MB 데이터에서 끝 100바이트와 5MB 지점 1KB만 읽으면 fetched는 블록 2~3개 분량
let mut r = BlockReader::new(MemRange::new(data_10mb), BLOCK_SIZE, MAX_BLOCKS);
r.seek(SeekFrom::End(-100)).unwrap(); r.read_exact(&mut [0;100]).unwrap();
r.seek(SeekFrom::Start(5_000_000)).unwrap(); r.read_exact(&mut [0;1024]).unwrap();
assert!(r.bytes_fetched() <= 3 * BLOCK_SIZE as u64);
// 읽은 내용이 원본과 같음, 블록 경계를 걸친 읽기 포함
// 같은 구간 재읽기는 fetch 증가 없음
// MAX_BLOCKS 초과 시 오래된 블록부터 버림
// 내부 read_at이 Err(ConnectionReset)을 내면 read가 같은 kind의 Err을 반환 (Review Focus 5)
```

`local.rs`: tempdir에 `a/b.zip`, `c.txt`를 만들고 `list_dir("a")`가 `b.zip`(is_dir=false, size 일치)을 돌려주는지, `open("a/b.zip")` 내용이 같은지.

- [ ] **Step 2:** `cargo test blockcache source::local` → FAIL.
- [ ] **Step 3:** 구현. `LocalSource::open`은 `std::fs::File`을 그대로 반환(로컬은 블록 캐시 불필요).
- [ ] **Step 4:** `cargo test blockcache source::local` → PASS.
- [ ] **Step 5:** 커밋 `feat: block cache reader and local source`

### Task 3: ZIP 인덱스 (권 구분·이미지 필터·인코딩)

**Files:** Create `src-tauri/src/archive.rs`, `src-tauri/tests/fixtures.rs`

**Interfaces:**
- Consumes: `encoding::decode_name`, `natsort::cmp`
- Produces:
  - `#[derive(Serialize, Deserialize, Clone)] struct PageRef { entry_index: usize, name: String }`
  - `#[derive(Serialize, Deserialize, Clone)] struct ArchiveVolume { inner_dir: Option<String>, pages: Vec<PageRef> }`
  - `#[derive(Serialize, Deserialize, Clone)] struct ZipIndex { volumes: Vec<ArchiveVolume> }`
  - `ZipIndex::read<R: Read + Seek>(r: R) -> Result<ZipIndex, ArchiveError>`
  - `read_entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, entry_index: usize) -> Result<Vec<u8>, ArchiveError>`
  - fixtures: `make_zip(entries: &[(&[u8] /*raw name*/, bool /*utf8 flag*/, &[u8] /*data*/)]) -> Vec<u8>`

규칙: 이미지 파일을 부모 폴더(전체 내부 경로)별로 묶는다. 이미지가 있는 폴더가 2개 이상이면 폴더마다 권(`inner_dir = Some(폴더 경로)`), 폴더 경로를 자연 정렬. 1개 이하면 권 하나(`inner_dir = None`). 권 안 페이지는 파일 이름 자연 정렬. 이미지 0개면 `volumes`가 빈 벡터.

- [ ] **Step 1: 실패하는 테스트**

```rust
// CP949 이름, 플래그 없음, 폴더 3개 + Thumbs.db + txt
let z = make_zip(&[(cp949("슬램덩크 완전판 10권/001.jpg"),false,JPG), (cp949("슬램덩크 완전판 02권/002.jpg"),false,JPG),
                   (cp949("슬램덩크 완전판 02권/001.jpg"),false,JPG), (cp949("슬램덩크 완전판 01권/001.jpg"),false,JPG),
                   (cp949("슬램덩크 완전판 01권/Thumbs.db"),false,b"x"), (b"readme.txt",false,b"x")]);
let idx = ZipIndex::read(Cursor::new(z)).unwrap();
assert_eq!(idx.volumes.iter().map(|v| v.inner_dir.clone().unwrap()).collect::<Vec<_>>(),
           ["슬램덩크 완전판 01권","슬램덩크 완전판 02권","슬램덩크 완전판 10권"]);
assert_eq!(idx.volumes[1].pages.iter().map(|p| &p.name).collect::<Vec<_>>(),
           ["슬램덩크 완전판 02권/001.jpg","슬램덩크 완전판 02권/002.jpg"]);
// 폴더 하나(쿄_01.zip 구조 "01/xxx.jpg") → volumes.len()==1, inner_dir None
// 루트에 이미지(생존게임 01.zip 구조) → volumes.len()==1
// __MACOSX/01/._001.jpg, 01/._001.jpg 포함 → 무시되어 volumes.len()==1 (Review Focus 1)
// 이미지 없음 → volumes 비어 있음
// 대문자 .JPG, .png 인식
// 1MB 크기 엔트리 200개짜리 ZIP을 BlockReader(MemRange)로 열면 index + 한 페이지 read_entry 후 bytes_fetched < 전체의 10%
```

- [ ] **Step 2:** `cargo test archive` → FAIL.
- [ ] **Step 3:** 구현. `zip::ZipArchive::new` 후 `by_index_raw(i)`의 `name_raw()`와 UTF-8 플래그(zip 9.x API는 context7로 확인)로 이름 디코딩. 디렉터리 엔트리 제외.
- [ ] **Step 4:** `cargo test archive` → PASS. 추가로 `#[ignore]` 테스트 `sample_folder_indexes`: 환경변수 `COMICS_SAMPLE_DIR`이 있으면 `슬램덩크 완전판.zip` 권 24개, `킹덤/킹덤 526~635화.zip` 권 110개, `생존게임/생존게임 01.zip` 권 1개를 확인. `COMICS_SAMPLE_DIR="/Users/gcjjyy/Downloads/QuickBASIC 4.5/만화" cargo test archive -- --ignored` → PASS.
- [ ] **Step 5:** 커밋 `feat: zip index with volume detection and cp949 names`

### Task 4: 서재 스캔과 작품·권 묶기

**Files:** Create `src-tauri/src/library.rs`

**Interfaces:**
- Consumes: `Source`, `DirEntry`, `ZipIndex`, `natsort::cmp`
- Produces:
  - `struct FoundZip { rel_path: String, size: u64, mtime: i64 }`
  - `fn find_zips(src: &dyn Source) -> io::Result<Vec<FoundZip>>` — 재귀, `.zip`/`.cbz`(대소문자 무시)만, 숨김 폴더(`.`로 시작) 제외
  - `#[derive(Serialize, Clone)] struct VolumeRef { key: String, zip_path: String, inner_dir: Option<String>, title: String, page_count: u32 }` — `key = zip_path` 또는 `zip_path + "#" + inner_dir`
  - `#[derive(Serialize, Clone)] struct Work { id: String, title: String, volumes: Vec<VolumeRef> }` — `id`는 작품 폴더 rel 경로, 루트 ZIP이면 그 ZIP rel 경로
  - `fn group_works(zips: &[FoundZip], index_of: &dyn Fn(&FoundZip) -> Option<ZipIndex>) -> Vec<Work>` — `None`(깨진 ZIP)과 빈 인덱스는 제외. 작품은 title 자연 정렬.

규칙(spec 5.2): ZIP의 부모 폴더가 루트(`""`)면 ZIP마다 작품(제목 = 파일명에서 확장자 뺀 것), 아니면 부모 폴더가 작품(제목 = 폴더 이름 마지막 부분). 작품 안 권 순서는 ZIP 파일명 자연 정렬, 다권 ZIP은 그 자리에서 안쪽 권을 순서대로 펼친다. 권 제목은 `inner_dir`의 마지막 부분, 없으면 ZIP 파일명(확장자 제외).

- [ ] **Step 1: 실패하는 테스트** (메모리 `FakeSource`로 트리 구성)

```rust
// 트리: 호문쿨루스.zip(다권 3), 생존게임/생존게임 {01,02,10}.zip, 이토준지/[완] 소용돌이 (1-3)/소용돌이 0{1,2,3}권.zip,
//       킹덤/킹덤636-642.zip(다권 7), 킹덤/킹덤 526~635화.zip(다권 110), 동물의 숲/a.jpg, broken.zip(index None), empty.zip(빈 인덱스)
let works = group_works(&find_zips(&src)?, &idx);
```
기대값:
- 작품 제목 목록(자연 정렬): `["[완] 소용돌이 (1-3)", "생존게임", "킹덤", "호문쿨루스"]` — `broken`, `empty`, `동물의 숲` 없음 (Review Focus 3)
- `생존게임` 권 제목: `["생존게임 01","생존게임 02","생존게임 10"]`
- `킹덤` 권 수 117, 첫 권 key가 `"킹덤/킹덤 526~635화.zip#킹덤 (KINGDIM)  (526)"` (파일명 자연 정렬상 `킹덤 526~635화` < `킹덤636-642`)
- `호문쿨루스` id `"호문쿨루스.zip"`, 권 3개

- [ ] **Step 2:** `cargo test library` → FAIL.
- [ ] **Step 3:** 구현.
- [ ] **Step 4:** `cargo test library` → PASS. `#[ignore]` 샘플 테스트: `COMICS_SAMPLE_DIR`로 `LocalSource` 스캔 → 작품에 `슬램덩크 완전판`(24권), `드래곤볼 완전판`, `번역판`이 있고 `동물의 숲 만화`는 없음. `-- --ignored` 실행 → PASS.
- [ ] **Step 5:** 커밋 `feat: library scan and work grouping`

### Task 5: 저장소(SQLite)와 방향 결정

**Files:** Create `src-tauri/src/schema.sql`, `src-tauri/src/db.rs`, `src-tauri/src/direction.rs`

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)] #[serde(rename_all="lowercase")] enum Direction { Rtl, Ltr }`
  - `direction::resolve(global: Direction, library: Option<Direction>, folder: Option<Direction>, book: Option<Direction>) -> Direction` — 가장 좁은 범위가 이김
  - `enum LibraryKind { Local, Sftp }`, `struct Library { id: i64, kind: LibraryKind, name: String, host: Option<String>, root: String, direction: Option<Direction> }`
  - `struct BookState { page: u32, page_count: u32, shift: bool, direction: Option<Direction> }`
  - `Db::open(path: &Path) -> Result<Db>`; `add_library(kind, name, host, root) -> i64`; `remove_library(id)`; `libraries() -> Vec<Library>`; `set_library_direction(id, Option<Direction>)`; `set_folder_direction(lib, work_id, Option<Direction>)`; `folder_direction(lib, work_id) -> Option<Direction>`; `book_state(lib, key) -> Option<BookState>`; `save_page(lib, key, page, page_count)`; `set_book_shift(lib, key, bool)`; `set_book_direction(lib, key, Option<Direction>)`; `global_direction() -> Direction`(기본 Rtl); `set_global_direction(Direction)`; `zip_index_cached(lib, rel, size, mtime) -> Option<ZipIndex>`; `put_zip_index(lib, rel, size, mtime, &ZipIndex)`; `works_cached(lib) -> Option<Vec<Work>>`; `put_works(lib, &[Work])`
  - `fn work_progress(works_volumes: &[VolumeRef], states: impl Fn(&str)->Option<BookState>) -> (u32 /*다 읽은 권*/, u32 /*전체*/)` — 마지막 페이지에 도달한 권을 다 읽은 것으로 셈

테이블: `settings(key PK, value)`, `libraries(id PK, kind, name, host, root, direction)`, `folder_settings(library_id, work_id, direction, PK(library_id, work_id))`, `book_state(library_id, volume_key, page, page_count, shift, direction, updated_at, PK(...))`, `zip_cache(library_id, rel_path, size, mtime, index_json, PK(...))`, `works_cache(library_id PK, works_json)`. `ON DELETE CASCADE`로 서재 삭제 시 함께 지움. 주석은 영어.

- [ ] **Step 1: 실패하는 테스트**

```rust
assert_eq!(resolve(Rtl, Some(Ltr), None, None), Ltr);
assert_eq!(resolve(Rtl, Some(Ltr), Some(Rtl), None), Rtl);
assert_eq!(resolve(Rtl, None, Some(Ltr), Some(Rtl)), Rtl);
assert_eq!(resolve(Rtl, None, None, None), Rtl);
// db: 임시 파일로 open → global_direction()==Rtl
// save_page 후 book_state 일치, 같은 key 재저장 시 덮어씀
// zip_index_cached: size나 mtime이 다르면 None
// remove_library 후 book_state/zip_cache 남지 않음
// 진행 기록만 있고 works에 없는 key는 work_progress 계산에 영향 없음 (Review Focus 4)
```

- [ ] **Step 2:** `cargo test db direction` → FAIL.
- [ ] **Step 3:** 구현. `schema.sql`은 `include_str!`, `PRAGMA foreign_keys=ON`.
- [ ] **Step 4:** `cargo test db direction` → PASS.
- [ ] **Step 5:** 커밋 `feat: sqlite store and direction resolution`

### Task 6: 앱 상태, 이미지 프로토콜, 썸네일, 커맨드 (로컬 서재 기준)

**Files:** Create `src-tauri/src/state.rs`, `protocol.rs`, `thumbs.rs`, `commands.rs`; Modify `lib.rs`; Create `src/api.ts`

**Interfaces:**
- Consumes: Task 1–5 전부
- Produces (Rust, 모두 `#[tauri::command]`, 오류는 `String`):
  - `list_libraries() -> Vec<LibraryView{ id, kind, name, host, root }>`
  - `add_local_library(path: String) -> i64` (이름 = 폴더 이름)
  - `remove_library(id: i64)`
  - `list_works(library_id: i64) -> Vec<WorkView{ id, title, volume_count, read_volumes, cover_url, direction }>` — 캐시가 있으면 바로 반환, 없으면 스캔 후 반환
  - `refresh_library(library_id: i64)` — 백그라운드 재스캔, 끝나면 바뀐 경우에만 이벤트 `library-updated { library_id }`
  - `list_volumes(library_id, work_id) -> Vec<VolumeView{ key, title, page_count, page, cover_url, direction }>`
  - `open_volume(library_id, key) -> OpenedVolume{ key, title, work_id, work_title, pages: Vec<String /*url*/>, page, shift, direction, prev_key: Option<String>, next_key: Option<String> }`
  - `save_progress(library_id, key, page: u32)`; `set_shift(library_id, key, shift: bool)`
  - `set_direction(scope: "global"|"library"|"folder"|"book", library_id: Option<i64>, target: Option<String>, direction: Option<Direction>)`
  - URI: `comic://localhost/page/{lib}/{enc(key)}/{index}`, `comic://localhost/thumb/{lib}/{enc(key)}` (Windows에서는 `http://comic.localhost/...`; `api.ts`의 `comicUrl(path)`가 플랫폼별로 만든다)
  - `AppState { db: Mutex<Db>, sources: Mutex<HashMap<i64, Arc<dyn Source>>>, open_archives: Mutex<LruCache<(i64,String), Arc<Mutex<ZipArchive<Box<dyn ReadSeek>>>>>> /* 4개 */, thumb_dir: PathBuf }`
- 썸네일: `thumbs::ensure(state, lib, key) -> PathBuf` — 첫 페이지를 디코딩해 가로 300px JPEG(품질 80)로 저장. 파일명 `sha1(lib|zip_path|inner_dir|size|mtime).jpg`. 원격 동시 생성은 2개로 제한(세마포어).
- 프로토콜 핸들러는 `register_asynchronous_uri_scheme_protocol`에서 `spawn_blocking`으로 처리, `Content-Type`은 확장자로, 응답에 `Cache-Control: max-age=31536000, immutable`.
- `api.ts`는 위 커맨드의 타입 래퍼와 `Direction = 'rtl'|'ltr'` 타입을 내보낸다.

- [ ] **Step 1: 실패하는 테스트** — `commands.rs`의 순수 로직을 분리한 `fn neighbor_keys(works: &[Work], key: &str) -> (Option<String>, Option<String>)` 테스트: 작품 안에서 앞/뒤 권, 첫 권의 prev는 None, 마지막 권의 next는 None, 없는 key는 (None, None).
- [ ] **Step 2:** `cargo test neighbor_keys` → FAIL.
- [ ] **Step 3:** 구현 + `lib.rs`에 상태·커맨드·프로토콜·dialog 플러그인 등록. capabilities에 dialog open 권한.
- [ ] **Step 4:** `cargo test` 전체 PASS. 수동 확인: `bun run tauri dev`, 개발자 도구 콘솔에서 `__TAURI__.core.invoke('add_local_library',{path:'/Users/gcjjyy/Downloads/QuickBASIC 4.5/만화'})` → `list_works` 결과에 `슬램덩크 완전판` 24권, `cover_url`을 `<img>`로 열면 표지가 보임.
- [ ] **Step 5:** 커밋 `feat: commands, comic:// protocol, thumbnails`

### Task 7: 책장 화면 (A안)

**Files:** Create `src/shelf/*.tsx`, `src/library/AddLibraryDialog.tsx`(로컬 탭만, SFTP 탭은 Task 10), Modify `src/App.tsx`, `src/styles.css`

**Interfaces:**
- Consumes: `api.ts`
- Produces: `<Progress value={0..1} direction={Direction} />` (rtl이면 오른쪽에서 왼쪽으로 참), `App`의 화면 상태 `{ view: 'shelf', libraryId, workId? } | { view: 'reader', libraryId, key }`

화면(시안 A 기준, 어두운 테마):
- 상단 막대: 앱 이름, 검색창(작품 제목 부분 일치, 대소문자·공백 무시).
- 사이드바: "서재" 목록(로컬은 폴더 아이콘, SFTP는 서버 아이콘 + 호스트 이름), `+ 서재 추가`, "최근 읽은 책". 서재 우클릭 메뉴: "읽는 방향: 일본 만화 / 한국 만화", "서재 삭제"(확인 대화상자: "'{이름}' 서재를 목록에서 뺍니다. 파일은 지우지 않습니다.").
- 작품 그리드: 표지(2:3, 겹친 책 효과), 권수 배지(`24권`), 제목, 진행 막대(`read_volumes/volume_count`). 우클릭 메뉴: "읽는 방향: 일본 만화 / 한국 만화 / 서재 설정 따름".
- 권 그리드: 뒤로(작품 목록), 권별 표지·제목·진행 막대(`page/page_count`). 클릭 시 읽기.
- 최근 읽은 책: `book_state.updated_at` 최신 20개 — `list_recent() -> Vec<VolumeView + library_id + work_title>` 커맨드를 이 태스크에서 추가.
- 서재를 열 때 `list_works`(캐시) 결과를 바로 그리고, 이어서 `refresh_library` 호출. `library-updated` 수신 시 다시 불러옴(그리드 크기 유지, 깜빡임 없음).
- 표지 이미지는 고정 비율 상자에 `loading="lazy"`, 로딩 중에는 빈 상자 색만.
- 서재가 하나도 없으면 가운데에 "만화 폴더를 서재로 추가하세요" + `서재 추가` 버튼.

- [ ] **Step 1: 실패하는 테스트** — `src/shelf/Progress.test.tsx`(vitest + @testing-library/react): `direction='rtl'`이면 채움 요소가 `right:0`에 붙고 `width: 50%`; `ltr`이면 `left:0`.
- [ ] **Step 2:** `bun run test` → FAIL.
- [ ] **Step 3:** 구현.
- [ ] **Step 4:** `bun run test` PASS. 수동: 샘플 폴더 서재 추가 → 작품 표지 그리드, `슬램덩크 완전판` 클릭 → 24권 그리드, 검색 "킹덤" → 1개. 스크린샷으로 확인.
- [ ] **Step 5:** 커밋 `feat: bookshelf UI`

### Task 8: 읽는 화면 (B안 두 쪽 펼침)

**Files:** Create `src/reader/spread.ts`, `keys.ts`, `usePrefetch.ts`, `Reader.tsx` + 테스트

**Interfaces:**
- Consumes: `open_volume`, `save_progress`, `set_shift`, `set_direction`
- Produces:
  - `type Dim = { w: number; h: number } | undefined`
  - `buildSpreads(count: number, dims: Dim[], shift: boolean, mode: 'spread' | 'single'): number[][]` — 각 원소는 읽는 순서의 페이지 인덱스 1~2개
  - `spreadIndexOf(spreads: number[][], page: number): number`
  - `screenOrder(spread: number[], dir: Direction): number[]` — rtl이면 뒤집어서 왼쪽→오른쪽 배치 순서로 반환
  - `actionForKey(e: { key: string; shiftKey: boolean }, dir: Direction): 'next' | 'prev' | null`
  - `actionForClick(xRatio: number, dir: Direction): 'next' | 'prev'` — 화면 왼쪽 절반 클릭: rtl→next, ltr→prev

짝짓기 규칙: 0번(표지) 혼자. `shift`면 1번도 혼자. 가로>세로인 페이지는 혼자이고, 짝짓기는 그 다음부터 다시. 짝이 될 다음 페이지가 가로면 현재 페이지 혼자. 남는 마지막 한 장은 혼자. `dims` 미확인(undefined)은 세로로 간주. `single` 모드는 모두 혼자.

- [ ] **Step 1: 실패하는 테스트**

```ts
expect(buildSpreads(6, [], false, 'spread')).toEqual([[0],[1,2],[3,4],[5]]);
expect(buildSpreads(6, [], true, 'spread')).toEqual([[0],[1],[2,3],[4,5]]);
const d = Array(7).fill({w:2,h:3}); d[3] = {w:6,h:4};
expect(buildSpreads(7, d, false, 'spread')).toEqual([[0],[1,2],[3],[4,5],[6]]);
const e = Array(5).fill({w:2,h:3}); e[2] = {w:6,h:4};
expect(buildSpreads(5, e, false, 'spread')).toEqual([[0],[1],[2],[3,4]]);
expect(buildSpreads(3, [], false, 'single')).toEqual([[0],[1],[2]]);
expect(spreadIndexOf([[0],[1,2],[3]], 2)).toBe(1);
expect(screenOrder([1,2], 'rtl')).toEqual([2,1]);
expect(screenOrder([1,2], 'ltr')).toEqual([1,2]);
expect(actionForKey({key:'ArrowLeft',shiftKey:false}, 'rtl')).toBe('next');
expect(actionForKey({key:'ArrowLeft',shiftKey:false}, 'ltr')).toBe('prev');
expect(actionForKey({key:'ArrowRight',shiftKey:false}, 'ltr')).toBe('next');
expect(actionForKey({key:' ',shiftKey:false}, 'ltr')).toBe('next');
expect(actionForKey({key:' ',shiftKey:true}, 'rtl')).toBe('prev');
expect(actionForClick(0.2, 'rtl')).toBe('next');
expect(actionForClick(0.2, 'ltr')).toBe('prev');
```

- [ ] **Step 2:** `bun run test` → FAIL.
- [ ] **Step 3:** 구현. `Reader.tsx`:
  - 페이지 `<img>`는 창 높이에 맞춤, 두 쪽은 가운데 맞닿게. 이미지 `onLoad`에서 `naturalWidth/Height`를 `dims`에 넣고 펼침 재계산. 재계산 시 현재 보고 있던 첫 페이지를 포함한 펼침으로 위치 유지.
  - 위 막대: `← 책장`, `작품 제목 · 권 제목`, 방향 칩(`日 오른쪽→왼쪽` / `韓 왼쪽→오른쪽`, 클릭 시 이 책 방향 전환), 보기 칩(`두 쪽`/`한 쪽`), `짝 밀기`. 아래 막대: 진행 막대(방향 따름), `현재/전체` 페이지. 둘 다 오버레이, 마우스 이동 시 표시, 2초 정지 후 숨김.
  - 단축키: 방향 `D`, 한 쪽/두 쪽 `S`, 짝 밀기 `A`, 전체 화면 `F`, 책장으로 `Esc`. 마우스 휠 아래 = 다음.
  - 마지막 펼침에서 다음 → `next_key`가 있으면 그 권을 0쪽부터 연다. 첫 펼침에서 이전 → `prev_key`의 마지막 페이지. 없으면 무시.
  - 페이지 바뀔 때 `save_progress`(300ms 디바운스, 페이지 = 펼침의 첫 페이지).
  - `usePrefetch(urls, currentPage)`: 앞 4장·뒤 2장을 `new Image()`로 미리 요청.
  - 이미지 응답 오류 시 그 자리에 "페이지를 불러오지 못했습니다" + 원격 서재면 `다시 연결` 버튼(Task 10의 `reconnect_library` 사용, 이 태스크에서는 버튼만 숨김 처리).
- [ ] **Step 4:** `bun run test` PASS. 수동: 슬램덩크 1권 열기 → 표지 혼자, `←`로 2·3쪽이 3쪽 왼쪽/2쪽 오른쪽, 한국 만화로 바꾸면 `→`가 다음. 마지막 쪽에서 다음 → 2권. 앱 재시작 후 같은 쪽에서 열림.
- [ ] **Step 5:** 커밋 `feat: two-page spread reader`

### Task 9: SSH 설정 해석

**Files:** Create `src-tauri/src/source/sftp/config.rs`; Cargo에 `ssh2-config` 추가

**Interfaces:**
- Produces:
  - `struct HostParams { alias: String, hostname: String, user: String, port: u16, identity_files: Vec<PathBuf>, identities_only: bool, extra_host_key_algs: Vec<String> }`
  - `fn list_hosts(config_text: &str) -> Vec<String>` — `*`, `?`, `!` 포함 패턴 제외, 등장 순서
  - `fn resolve(alias: &str, config_text: &str, home: &Path, local_user: &str) -> HostParams` — 기본값 port 22, user = 로컬 사용자, hostname = alias, `IdentityFile`의 `~` 확장. `IdentityFile`이 없으면 `~/.ssh/id_ed25519`, `id_ecdsa`, `id_rsa` 중 존재하는 것.

- [ ] **Step 1: 실패하는 테스트**

```rust
let cfg = "Host nas\n  HostName 10.0.0.5\n  User me\n  Port 2222\n  IdentityFile ~/.ssh/nas_key\n  IdentitiesOnly yes\n\
           Host old\n  HostName old.example\n  HostKeyAlgorithms +ssh-rsa\n\nHost *\n  ServerAliveInterval 30\n";
assert_eq!(list_hosts(cfg), ["nas","old"]);
let p = resolve("nas", cfg, Path::new("/h"), "u");
assert_eq!((p.hostname.as_str(), p.user.as_str(), p.port), ("10.0.0.5","me",2222));
assert_eq!(p.identity_files, [PathBuf::from("/h/.ssh/nas_key")]); assert!(p.identities_only);
let o = resolve("old", cfg, tmp_home_with_id_ed25519, "u");
assert_eq!(o.extra_host_key_algs, ["ssh-rsa"]); assert_eq!(o.port, 22); assert_eq!(o.user, "u");
assert_eq!(o.identity_files, [tmp_home.join(".ssh/id_ed25519")]);
```

- [ ] **Step 2:** `cargo test sftp::config` → FAIL.
- [ ] **Step 3:** 구현. `ssh2-config`가 `+ssh-rsa` 형식이나 기본값을 다르게 다루면 그 부분만 직접 처리(context7/docs.rs 확인).
- [ ] **Step 4:** `cargo test sftp::config` → PASS. 사용자의 실제 `~/.ssh/config`로 `list_hosts`가 4개 호스트를 돌려주는지 `#[ignore]` 테스트로 확인.
- [ ] **Step 5:** 커밋 `feat: ssh config host resolution`

### Task 10: SFTP 접속·인증·소스와 서재 추가 UI

**Files:** Create `src-tauri/src/source/sftp/{mod.rs, session.rs, file.rs, prompt.rs}`, `src/library/RemoteBrowser.tsx`, `src/library/SshPromptDialog.tsx`; Modify `AddLibraryDialog.tsx`, `commands.rs`, `state.rs`, `Reader.tsx`(다시 연결 버튼 연결); Cargo에 `russh`, `russh-sftp`, `keyring` 추가

**Interfaces:**
- Consumes: `HostParams`, `RangeRead`, `BlockReader`, `Source`
- Produces:
  - `trait Prompter: Send + Sync { fn confirm_host_key(&self, host: &str, fingerprint: &str) -> bool; fn ask_secret(&self, kind: SecretKind, host: &str) -> Option<(String, bool /*remember*/)>; }`, `enum SecretKind { KeyPassphrase { key_path: String }, Password }`
  - `TauriPrompter` — 이벤트 `ssh-prompt { id, kind: 'hostkey'|'passphrase'|'password', host, fingerprint?, key_path? }`를 보내고 커맨드 `ssh_prompt_reply(id: u64, accept: bool, secret: Option<String>, remember: bool)`의 응답을 oneshot으로 기다림(5분 후 거절로 간주)
  - `SftpSource::connect(params: HostParams, root: String, prompter: Arc<dyn Prompter>) -> Result<SftpSource>` — `Source` 구현, `open`은 `BlockReader<SftpRange>`를 `Box<dyn ReadSeek>`로 반환
  - `SftpRange` — `RangeRead` 구현, 전용 tokio 런타임 핸들로 `block_on`(호출은 항상 blocking 스레드에서)
  - 커맨드: `list_ssh_hosts() -> Vec<String>`, `sftp_list_dir(host: String, path: String) -> Vec<DirEntryView{ name, is_dir }>`(폴더만 + zip 개수), `add_sftp_library(host: String, root: String) -> i64`(이름 = `host:root 마지막 폴더`), `reconnect_library(library_id) -> ()`

동작:
- 인증 순서: ssh-agent의 키 → `identity_files`(암호 걸린 키면 `KeyPassphrase` 질문) → `Password` 질문. `IdentitiesOnly yes`면 agent 키 중 `identity_files`와 공개키가 같은 것만. 비밀번호·암호를 "기억"하면 `keyring`(서비스명 `comics-viewer`, 계정 `host` 또는 키 경로)에 저장하고 다음에 먼저 시도.
- 호스트 키: `~/.ssh/known_hosts`(포트가 22가 아니면 `[host]:port` 형식) 확인. 없으면 `confirm_host_key`(지문 SHA256) 승인 시 known_hosts에 추가. 다르면 접속 거부, 오류 문구 "'{host}' 서버의 호스트 키가 known_hosts에 저장된 것과 다릅니다. 서버가 바뀐 게 맞다면 터미널에서 `ssh-keygen -R {host}` 실행 후 다시 시도하세요."
- `extra_host_key_algs`에 `ssh-rsa`가 있으면 클라이언트 선호 호스트 키 알고리즘에 `ssh-rsa`(SHA-1)를 추가.
- 읽기 중 연결 오류: `SftpSource`가 한 번 재연결 후 같은 요청 재시도, 그래도 실패하면 오류 반환(멈추지 않음).
- 서재 추가 대화상자: 탭 `내 컴퓨터` / `SSH 서버`. SSH 탭은 호스트 드롭다운(`~/.ssh/config` 목록) → 접속 → `RemoteBrowser`로 폴더 탐색(빵부스러기 경로, 폴더 행에 하위 ZIP 수 없음·폴더만) → `이 폴더를 서재로 추가`.
- `SshPromptDialog`: 호스트 키는 "'{host}' 서버에 처음 접속합니다. 지문: {fp}. 이 서버가 맞으면 접속하세요." + `접속`/`취소`. 암호는 입력칸 + `기억하기` 체크 + `확인`/`취소`.

- [ ] **Step 1: 실패하는 테스트**
  - `file.rs`: `SftpRange`를 흉내 낸 `FlakyRange`(n번째 호출에 `ConnectionReset`)를 감싼 `ReconnectingRange`가 재연결 콜백 1회 호출 후 성공하는지, 두 번 연속 실패면 Err을 반환하는지.
  - `session.rs`: `known_host_status(known_hosts_text, host, port, key) -> Known | Unknown | Mismatch` 순수 함수 테스트(22 포트는 `host`, 그 외는 `[host]:port` 매칭, 해시된 항목 `|1|...` 지원).
- [ ] **Step 2:** `cargo test sftp` → FAIL.
- [ ] **Step 3:** 구현(russh/russh-sftp API는 context7로 확인. ssh-rsa·agent·암호 걸린 OpenSSH 키 중 하나라도 russh로 안 되면 이 태스크에서 `ssh2`로 교체하고 계획에 기록).
- [ ] **Step 4:** `cargo test sftp` PASS. 통합: `#[ignore]` 테스트 `sftp_reads_partial` — 환경변수 `COMICS_SFTP_HOST`, `COMICS_SFTP_ZIP`(원격 큰 ZIP 경로)로 접속, `ZipIndex::read` + 첫 페이지 `read_entry` 후 `bytes_fetched`가 파일 크기의 5% 미만. 사용자에게 호스트와 경로를 받아 실행. 수동: 앱에서 SSH 서재 추가 → 책장 표시 → 권 열기·넘기기.
- [ ] **Step 5:** 커밋 `feat: sftp libraries via ~/.ssh/config`

### Task 11: 마무리 확인과 빌드

**Files:** Modify 필요 시 `tauri.conf.json`(번들 이름·아이콘)

- [ ] **Step 1:** 전체 테스트: `cd src-tauri && cargo test && cd .. && bun run test` → PASS. 샘플 테스트 `COMICS_SAMPLE_DIR=... cargo test -- --ignored` → PASS.
- [ ] **Step 2:** `bun run tauri build` → macOS `.app`/`.dmg` 생성, 크기 확인(목표 20MB 이하).
- [ ] **Step 3:** 빌드한 앱으로 시나리오 확인: 샘플 서재 추가 → 표지 로딩 → `슬램덩크 완전판` 3권 중간 → 재시작 후 이어 보기 → 작품 폴더 한국 만화 지정 시 진행 막대·키 방향 변경.
- [ ] **Step 4:** 커밋 `chore: release build config`
