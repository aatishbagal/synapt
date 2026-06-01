# Synapt Flutter Codebase Audit

This document is a read-only audit of the legacy Flutter implementation in `synapt-legacy/`. It records what exists, how it works, and what is incomplete, to serve as a reference for the Rust rewrite. No Rust is written here and no legacy file was modified.

## Project Overview

Synapt is a desktop "spotlight"-style launcher and multi-device file search utility. A global hotkey (Ctrl+Space) summons a transparent always-on-top search overlay. The user types a query and the app searches a locally built file index, returning ranked results that can be opened. Queries prefixed with `@` switch to device/command mode (for example `@settings` or `@<device_name>` to target another of the user's devices), queries prefixed with `/` search folders only, and bare arithmetic expressions are evaluated inline as a calculator. Devices belonging to the same authenticated user discover each other over the LAN via UDP multicast, can run searches against each other over TCP, and can pull files peer-to-peer. Account and device registry live in a Supabase backend; the local file index is held in memory.

### Flutter and Dart constraints

- Dart SDK constraint: `>=3.0.0 <4.0.0` (from `pubspec.yaml`).
- Flutter: not pinned in `pubspec.yaml`; README badges state Flutter 3.x and Dart 3.x.
- App version: `0.8.7+4`.

### Dependencies

From `pubspec.yaml` runtime dependencies:

- `sqflite` ^2.3.0 — SQLite plugin (mobile-style API).
- `sqflite_common_ffi` ^2.3.0 — SQLite via FFI for desktop platforms.
- `path` ^1.8.3 — path manipulation utilities.
- `permission_handler` ^11.1.0 — runtime permission requests.
- `hotkey_manager` ^0.2.3 — global system hotkey registration.
- `desktop_multi_window` ^0.2.0 — spawns the separate Settings window.
- `supabase_flutter` ^2.10.3 — auth and Postgres/Realtime backend client.
- `url_strategy` ^0.2.0 — URL strategy helper (web routing).
- `url_launcher` ^6.2.2 — open external URLs.
- `device_info_plus` ^9.1.1 — host/OS/device name detection.
- `collection` ^1.17.2 — collection helpers.
- `crypto` ^3.0.3 — HMAC-SHA256 used to sign discovery packets.
- `provider` ^6.1.1 — state management (ChangeNotifier).
- `window_manager` ^0.3.7 — frameless transparent window control.
- `uuid` ^4.5.1 — UUID generation (declared; no direct use found in `lib/`).
- `shared_preferences` ^2.2.2 — key/value settings store.
- `math_expressions` ^2.4.0 — calculator expression parsing.
- `screen_retriever` ^0.1.9 — multi-monitor cursor/display geometry.
- `path_provider` ^2.1.1 — app support directory for the JSON index cache.

Commented-out (not active): `firebase_core`, `firebase_auth`, `google_sign_in`.

Dev dependencies: `flutter_test`, `flutter_lints` ^3.0.1, `integration_test`.

### Dart file count

`lib/` contains 32 `.dart` files. No generated files (`.g.dart`, `.freezed.dart`) are present.

### Top-level structure of lib/

```
lib/
├── config/      # Supabase config (example template only)
├── core/        # Data structures, search engine, indexer
├── depricated/  # One retired widget (folder is misspelled in the source)
├── models/      # Device, File, SearchResult data models
├── services/    # Auth, database, discovery, search, storage, hotkey
├── ui/          # Screens, widgets, theme
├── utils/       # App version, config manager
└── main.dart    # Entry point, window/hotkey setup, active search overlay
```

## Screen and Feature Inventory

The app has no router. It is not a conventional multi-screen app: the main window is a 1x1 transparent frameless window that stays hidden, and UI is shown either as a dialog over an invisible navigator (the search overlay) or in a second OS window (settings) spawned through `desktop_multi_window`. Navigation is therefore done with `showDialog`, `Navigator.pop`, and `DesktopMultiWindow.createWindow`, not named routes.

### Main screens

- Search Overlay (`SearchOverlayDialog` in `main.dart`). The primary surface. Centered 640px panel summoned by the global hotkey. The user types to search local files, evaluate arithmetic, or issue `@`/`/` commands; results render in a selectable list with file-type icons. Actions: type to search; arrow up/down to move selection; Enter to open the selected result (or copy a calculator result to the clipboard); Esc or tap-outside to dismiss; hover to select; click a result to execute; selecting a device result retargets subsequent searches to that device; selecting "Open Settings" opens the settings window. A download progress bar appears during remote file transfer.
- Auth Screen (`ui/screens/auth_screen.dart`). Shown inside the settings window when the user is not authenticated. Email/password sign-in and sign-up with show/hide password and inline validation; a "Continue with Google" button is present but documented as non-functional. Actions: enter email/password; submit sign-in or sign-up; toggle between sign-in and sign-up; attempt Google sign-in.
- Settings Screen (`ui/screens/settings_screen.dart`). Shown in the spawned settings window when authenticated. Sections: account profile (with edit-display-name dialog), global hotkey rebind, this-device info, connected-devices list with online/offline status and IP, a debug section (mock-devices toggle and indexed-directory/file-count listing), version info, and sign-out. Actions: edit display name; record a new hotkey; toggle mock devices; refresh devices; sign out.

### Dialogs and overlays

- Search overlay dialog (`showDialog` with a transparent barrier) — described above.
- Edit Display Name dialog (`AlertDialog` inside settings) — rename with 3-30 char validation.
- Settings window — a separate top-level OS window rather than an in-app route, launched via `DesktopMultiWindow`.

### Shared widgets

- `SynaptSearchBar` (`ui/widgets/search_bar.dart`) — text field with an optional selected-device chip and clear button. Used by both overlay implementations.
- `ResultList` (`ui/widgets/result_list.dart`) — scrolling list that auto-scrolls to keep the selected item visible; builds `FileItem` rows. Used only by the unused `SearchOverlay`.
- `FileItem` (`ui/widgets/file_item.dart`) — single result row (emoji icon, title, subtitle, remote chip or age). Used only by `ResultList`.

### Unused or legacy UI

- `ui/screens/search_overlay.dart` (`SearchOverlay`) — a second, older overlay implementation using `ResultList`/`FileItem`. The active overlay is `SearchOverlayDialog` in `main.dart`; `SearchOverlay` does not appear to be routed anywhere.
- `ui/screens/simple_test.dart` (`SimpleTestScreen`) — a plain debug search screen, not wired into the app.
- `depricated/enhanced_search_bar.dart` (`EnhancedSearchBar`) — retired search bar (342 lines), not imported by active code.

## Network Protocol

Networking spans three files: `services/network_discovery_service.dart` (LAN presence), `services/remote_search_service.dart` (TCP search and file transfer), and `services/database_service.dart` (Supabase device registry and online heartbeat). Note: the README architecture diagram labels device communication as "SSH Protocol"; this is not accurate. The actual transport is plaintext TCP carrying JSON, with no encryption or authentication on the search/transfer channel.

### Peer Discovery

- Protocol: UDP multicast. The socket binds to the selected interface address on the multicast port, joins the multicast group, enables broadcast, and disables multicast loopback.
- Address and port: multicast group `239.255.42.99`, port `42099` (`multicastAddress`, `multicastPort`).
- Interface selection: `_selectBestNetworkInterface` enumerates IPv4 non-loopback, non-link-local interfaces and scores them (favoring `192.168.1.x`/`192.168.0.x` and Wi-Fi/Ethernet names; penalizing virtual adapters such as VMware, VirtualBox, Docker, WSL, Hyper-V, and `169.254.x` link-local).
- Packet format: UTF-8 JSON, a single object:

```dart
final message = {
  'type': 'presence',
  'user_id': userId,
  'device_id': deviceId,
  'timestamp': DateTime.now().millisecondsSinceEpoch,
  'signature': _generateSignature(userId, deviceId),
};
```

- Signature: HMAC-SHA256 over `"$userId:$deviceId"`, keyed by the current user's email (falling back to the literal `'synapt-key'`). The receiver does not verify the signature; it only checks that `user_id` matches the local user and that `device_id` is not itself.
- Peer add/expire: on receiving a `presence` packet from the same user (and a different device), the sender IP and `lastSeen` are stored in an in-memory `_discoveredDevices` map keyed by `device_id`. Presence is broadcast every 5 seconds (`broadcastInterval`). A cleanup timer runs every 5 seconds and removes devices not seen within 20 seconds (`deviceTimeout`).
- Retry behavior: none beyond the periodic 5-second rebroadcast; no acknowledgements.

### File Transfer

- Transport: plaintext TCP (`dart:io` `Socket`/`ServerSocket`).
- Port: `42100` (`remoteSearchPort`) — the same port is used for both remote search and file transfer requests.
- Search flow: client connects (2s `connectionTimeout`), writes one newline-terminated JSON request, and reads until it sees a newline or the socket closes (3s `searchTimeout`). Request and response are parsed by locating the first `{` and last `}` in the buffer.

```dart
final request = {
  'type': 'search',
  'query': query,
  'max_results': 50,
  'timestamp': DateTime.now().millisecondsSinceEpoch,
};
// response: { 'type': 'search_results', 'results': [ ... ], 'timestamp': ... }
```

- File transfer flow: client connects to the same port and sends `{ 'type': 'file_transfer', 'file_path': remotePath, 'timestamp': ... }` followed by a newline. The server replies with a newline-terminated JSON header `{ 'type': 'file_data', 'size': <bytes>, 'timestamp': ... }` and then streams the raw file bytes via `file.openRead().pipe(client)`. The client writes incoming bytes to `~/Downloads/Synapt/<deviceName>/<filename>`, tracking `bytesReceived` against the header `size` and stopping when it reaches that size.
- Framing: a single newline separates the JSON header from the binary payload; there is no length-delimited chunk framing for the body, only the total size in the header.
- Large files: streamed, not chunked at the application layer; no per-chunk acknowledgement. The indexer separately skips files over 10 MB at index time (see Data Model), but transfer itself imposes no size cap.
- Resume on disconnect: not implemented. A dropped connection aborts the transfer and the partial file is left in place.
- Progress: the client computes `bytesReceived / fileSize` and invokes an `onProgress` callback, surfaced as a percentage and progress bar in the overlay during download.
- Error handling: connection/timeout/parse errors are caught and generally result in an empty result list or a `null` transfer path; the server replies with `{ 'type': 'error', 'message': ... }` for invalid requests or missing files. Errors are logged via `debugPrint` and largely swallowed.

### Peer State

- A discovered peer (`_DiscoveredDevice`, in-memory) holds `deviceId`, `ipAddress`, `lastSeen`.
- The UI-facing `DeviceModel` holds `id`, `name`, `type`, `isOnline`, `lastSeen`, `platform`, `isLocalNetwork`, `ipAddress`.
- The Supabase-backed `DeviceInfo` holds `id`, `userId`, `deviceName`, `deviceAlias`, `deviceType`, `operatingSystem`, `synaptVersion`, `localIp`, `macAddress`, `hostname`, `isOnline`, `lastSeen`, `createdAt`, and a transient `isLocalNetworkOnline`.
- Storage: peer/device records live in Supabase (`devices` table); LAN presence and IPs are in-memory only. `DeviceService` merges the Supabase device list with live discovery state, treating "online" as "currently seen on the LAN" (a device is shown online only if discovery has a recent presence packet for it).
- Offline handling: discovery drops a peer after 20s of silence and notifies listeners; `DeviceService` recomputes each device's `isOnline`/`isLocalNetwork`/`ipAddress`. The local device sends a Supabase heartbeat every 30 seconds and marks itself offline on demand via `setDeviceOffline`.

## DSA Implementations

The four data structures live under `lib/core/`, not the paths suggested in the audit brief. Found at: `lib/core/search/trie.dart`, `lib/core/storage/bloom_filter.dart`, `lib/core/storage/lru_cache.dart`, `lib/core/search/fuzzy_matcher.dart`. There is no `levenshtein.dart`; Levenshtein lives inside the fuzzy matcher. All four are pure Dart with no external dependencies and are usable as a reference.

### Trie (`lib/core/search/trie.dart`)

Exists. Implements a generic prefix tree `Trie<T>` over a `TrieNode<T>`, storing associated data and per-word frequency, with prefix collection and autocompletion. Keys are lowercased on insert and search.

Public API:
- `TrieNode<T>.insert(String word, T data)` — insert a word and attach data at the terminal node.
- `TrieNode<T>.search(String prefix) -> List<T>` — collect all data under a prefix.
- `TrieNode<T>.getAutoCompletions(String prefix, {int maxSuggestions=10}) -> List<String>` — completion strings under a prefix.
- `TrieNode<T>.getFrequency(String word) -> int` — frequency of an exact word.
- `TrieNode<T>.remove(String word) -> bool` — remove a word, pruning empty nodes.
- `Trie<T>.insert(String word, T data)` — wrapper that also maintains `_size`.
- `Trie<T>.search(String prefix) -> List<T>`, `getAutoCompletions(...)`, `contains(word) -> bool`, `getFrequency(word) -> int`, `remove(word) -> bool`, `size`, `isEmpty`, `clear()`.

Core logic:

```dart
void insert(String word, T data) {
  TrieNode<T> current = this;
  word = word.toLowerCase();
  for (int i = 0; i < word.length; i++) {
    final char = word[i];
    current.children.putIfAbsent(char, () => TrieNode<T>());
    current = current.children[char]!;
  }
  current.isEndOfWord = true;
  current.frequency++;
  if (!current.associatedData.contains(data)) {
    current.associatedData.add(data);
  }
}
```

Known issues: the file contains two top-level definitions of `class Trie<T>`. The first is the formatted class (lines 146-195); a second, byte-identical, minified copy is appended on line 196 directly after the first class's closing brace. As written, two classes named `Trie<T>` in one library is a duplicate-declaration compile error; this should be verified against whatever source actually builds. `search` does a full subtree walk (`_collectAllData`) on every query with no result cap, and `associatedData.contains` makes inserts linear in the number of items sharing a terminal node.

### Bloom Filter (`lib/core/storage/bloom_filter.dart`)

Exists. Implements a standard bit-array Bloom filter with optimal size/hash-count computation, plus two extensions: `FileBloomFilter` (adds normalized path, filename, and extension) and `CountingBloomFilter` (supports removal via integer counters).

Public API (`BloomFilter`):
- `BloomFilter({required int expectedElements, required double falsePositiveRate})`.
- `add(String element)`, `addAll(Iterable<String>)`.
- `contains(String element) -> bool`.
- `filterPossibleMatches(Iterable<String>) -> List<String>`.
- `getCurrentFalsePositiveRate() -> double`, `getStats() -> Map`, `clear()`.
- `union(BloomFilter) -> BloomFilter`, `intersection(BloomFilter) -> BloomFilter`.

`FileBloomFilter` adds `addFilePath(String)` and `mightContainFile(String)`. `CountingBloomFilter` adds `add`, `remove`, `contains`, `clear`, `getStats`.

Core logic:

```dart
List<int> _getHashValues(String element) {
  final bytes = utf8.encode(element);
  final hash1 = _fnv1aHash(bytes);  // FNV-1a
  final hash2 = _djb2Hash(bytes);   // DJB2
  final hashes = <int>[];
  for (int i = 0; i < _hashCount; i++) {
    hashes.add((hash1 + i * hash2).abs()); // double hashing
  }
  return hashes;
}
```

Known issues: no explicit TODOs. The optimal size/hash-count are computed three times in the constructor initializer list (redundant recomputation). `CountingBloomFilter` allocates one `int` per bit position (`List.filled(_size, 0)`), which is far larger than the bit-packed `BloomFilter`. The filter is built and populated by the search engine but never actually queried during search (see Known Issues).

### LRU Cache (`lib/core/storage/lru_cache.dart`)

Exists. Classic O(1) LRU using a `HashMap` plus a doubly linked list with sentinel head/tail nodes. `SearchResultCache<K,V>` extends it to track access times and counts.

Public API (`LRUCache`):
- `LRUCache(int capacity)`.
- `get(K) -> V?`, `put(K, V)`, `containsKey(K) -> bool`, `remove(K) -> V?`.
- `keys`, `values`, `length`, `capacity`, `isEmpty`, `isFull`, `clear()`.

`SearchResultCache` adds `getLastAccessTime`, `getAccessCount`, `getMostFrequentItems({int limit=10})`, `getItemsAccessedSince(DateTime)`.

Core logic:

```dart
void put(K key, V value) {
  final existingNode = _cache[key];
  if (existingNode != null) {
    existingNode.value = value;
    _moveToHead(existingNode);
  } else {
    final newNode = _LRUNode(key, value);
    if (_cache.length >= _capacity) {
      final tail = _removeTail();
      if (tail.key != null) _cache.remove(tail.key);
    }
    _cache[key] = newNode;
    _addToHead(newNode);
  }
}
```

Known issues: `keys`/`values` skip nodes whose key or value is null, so null values cannot be stored or iterated reliably; otherwise complete.

### Fuzzy Matcher (`lib/core/search/fuzzy_matcher.dart`)

Exists. A multi-algorithm string-similarity library exposing Levenshtein, Damerau-Levenshtein, Jaro, Jaro-Winkler, longest-common-subsequence, cosine (character n-gram), and a weighted `combined` score. Includes `FuzzyMatch`/`FuzzyHighlight` result types and a `FuzzyAlgorithm` enum.

Public API:
- `calculateScore(String query, String target, {FuzzyAlgorithm algorithm=levenshtein, bool caseSensitive=false}) -> double`.
- `findBestMatches(String query, List<String> candidates, {int maxResults=10, double threshold=0.6, FuzzyAlgorithm algorithm=combined, bool caseSensitive=false}) -> List<FuzzyMatch>`.
- `isSimilar(String, String, {double threshold=0.6, FuzzyAlgorithm algorithm=levenshtein}) -> bool`.

Core logic:

```dart
int _levenshteinDistance(String s1, String s2) {
  if (s1.isEmpty) return s2.length;
  if (s2.isEmpty) return s1.length;
  final matrix = List.generate(s1.length + 1, (i) => List.filled(s2.length + 1, 0));
  for (int i = 0; i <= s1.length; i++) matrix[i][0] = i;
  for (int j = 0; j <= s2.length; j++) matrix[0][j] = j;
  for (int i = 1; i <= s1.length; i++) {
    for (int j = 1; j <= s2.length; j++) {
      final cost = s1[i - 1] == s2[j - 1] ? 0 : 1;
      matrix[i][j] = math.min(math.min(matrix[i-1][j]+1, matrix[i][j-1]+1), matrix[i-1][j-1]+cost);
    }
  }
  return matrix[s1.length][s2.length];
}
```

Known issues: no TODOs. `_generateHighlights` is explicitly noted in-code as simplified (substring/bigram only). All distance algorithms allocate full O(m*n) matrices, and the combined score runs four algorithms per comparison, which is expensive when the search engine applies it across the whole file set.

## Data Model and Storage

### Data Models

`FileModel` (`models/file_model.dart`): `id` (String, hash of path), `name`, `path`, `parentPath`, `type` (`FileType` enum: file/directory/symlink), `size` (int), `lastModified`, `lastAccessed`, `created` (DateTime), `mimeType`, `extension`, `isHidden`, `isSymlink` (bool), `metadata` (`Map<String,dynamic>?`, used to carry extracted text `content`). Has `formattedSize`, `ageString`, and a `category` getter (`FileCategory` enum). Equality is by `path`. JSON round-trips for SQLite persistence.

`ApplicationModel` (same file): `id`, `name`, `path`, `description`, `version`, `icon`, `type` (`ApplicationType` enum), `lastUsed`, `usageCount`. Defined and persistable but not populated anywhere in the indexing flow.

`DeviceModel` (`models/device_model.dart`): `id`, `name`, `type`, `isOnline`, `lastSeen`, `platform`, `isLocalNetwork`, `ipAddress`. The runtime/UI view of a device. In-memory only.

`DeviceInfo` (`services/database_service.dart`): the Supabase row model — `id`, `userId`, `deviceName`, `deviceAlias`, `deviceType`, `operatingSystem`, `synaptVersion`, `localIp`, `macAddress`, `hostname`, `isOnline`, `lastSeen`, `createdAt`, transient `isLocalNetworkOnline`. Persisted in the `devices` table.

`SearchResult` (`models/search_result.dart`): `id`, `title`, `subtitle`, `path`, `type` (`SearchResultType` enum), `relevanceScore`, `metadata`, `lastAccessed`, `accessCount`, `highlights` (`List<MatchHighlight>`), `deviceId`, `deviceName`, `isRemote`. In-memory and wire-serialized for remote search; not persisted. Carries its own scoring factories and JSON for the TCP protocol. `SearchResultSet` wraps a result list with query, timing, totals, merge/sort/filter helpers.

There is no dedicated `Settings` model; preferences are loose keys (see below).

### Persistence

Three separate persistence mechanisms exist, and they are only partly wired up:

- SQLite via `sqflite`/`sqflite_common_ffi` (`core/storage/database.dart`, database file `synapt.db`). Schema version 1 creates three tables: `files` (id, name, path unique, parent_path, type, size, last_modified, last_accessed, created, mime_type, extension, is_hidden, is_symlink, metadata) with indexes on name/path/extension; `applications` (id, name, path unique, description, version, icon, type, last_used, usage_count) with a name index; and `search_history` (id autoincrement, query, timestamp, result_count). `StorageService` exposes `saveFile`, `getAllFiles`, `saveApplication`, `getAllApplications`. However, the live indexing path (`SearchService.scanFileSystem`) feeds the in-memory `SearchEngine` only and never calls `StorageService.saveFile`; the SQLite tables are effectively unused at runtime.
- Supabase (Postgres, `backend/schema.sql`). Tables `public.users` (id, email, display_name, avatar_url, timestamps) and `public.devices` (id, user_id, device_name, device_alias, device_type, operating_system, synapt_version, hostname, local_ip, mac_address, is_online, last_seen, created_at), both with row-level security policies, and a Realtime publication on `devices`. This is the live store for accounts and the device registry.
- JSON file cache (`services/indexed_directories_cache.dart`). Writes `indexed_directories.json` (directory list, file count, timestamp) into the app support directory; reloaded on startup to display previously indexed directories before a rescan.
- SharedPreferences (`utils/config_manager.dart`). A thin settings wrapper with keys `max_results` (default 50), `include_hidden` (default false), `theme_mode` (default `system`). `ConfigManager.initialize` is not called from `main.dart`, so these defaults are effectively the only values in use.

### State Management

Provider (`ChangeNotifier`) throughout, with one `MultiProvider` in `SynaptApp` and a duplicate set in the settings window. Major providers:

- `SearchService` — owns local index lifecycle, indexed-directory list, current-device targeting, transfer progress, and dispatch between local and remote search.
- `AuthService` — Supabase auth state, sign-in/up/out, profile, error/loading flags.
- `DatabaseService` — Supabase device registry, current device registration, heartbeat, realtime subscription.
- `DeviceService` — merged view of DB devices plus LAN discovery state, selected device, mock-device mode.
- `NetworkDiscoveryService` — UDP multicast presence and the discovered-device map.
- `RemoteSearchService` — TCP client and server for remote search and file transfer.
- `HotkeyService` — current global hotkey and the recording state machine for rebinding.
- `StorageService` — SQLite-backed file/app persistence (provided but largely unused).

Non-Provider singletons: `SynapseDatabase` (static DB handle) and `IndexedDirectoriesCache` (static JSON cache).

## Known Issues and Incomplete Areas

No `TODO`, `FIXME`, or `UnimplementedError` markers exist anywhere in `lib/`. The issues below are inferred from the code and from README/architecture statements.

- `lib/core/search/trie.dart:196` — duplicate `class Trie<T>` (a minified copy appended after the formatted class). As written this is a duplicate top-level declaration and would not compile; needs verification against the building source. Severity: blocks core function if real.
- `lib/core/search/search_engine.dart` (indexFile/search) — the `FileBloomFilter` is populated (`_bloomFilter.addFilePath`) but `search()` never calls it to short-circuit lookups, so the bloom filter contributes nothing but build cost. Severity: degrades experience.
- `lib/services/search_service.dart:96-112` — indexed files go only into the in-memory `SearchEngine`; `StorageService`/SQLite is never written, so the entire index is rebuilt from disk on every launch. Severity: degrades experience.
- `lib/core/search/search_engine.dart:260-333` — `_searchFileContent` and `_fuzzySearch` both iterate the full `_filesByPath` map per query (O(n) over all files, with the four-algorithm fuzzy score per file), which will not scale toward the documented 10k-file target. `_findFuzzyWordMatch` similarly scans every inverted-index key. Severity: degrades experience.
- `lib/core/indexer/file_scanner.dart:90` vs `lib/services/search_service.dart:83` — the scanner default `maxFiles` is 50000 but the caller passes 10000; the README states a 10k limit. Inconsistent caps. Severity: cosmetic.
- `lib/services/network_discovery_service.dart:323-328` and `remote_search_service.dart` — presence packets are signed with HMAC but the signature is never verified, and the TCP search/transfer channel has no authentication or encryption despite the README/architecture calling it "SSH Protocol." Any host on the LAN claiming the same `user_id` can be trusted, and file transfer serves any requested absolute path. Severity: blocks core function for a security-sensitive feature; at minimum a major gap.
- `lib/services/remote_search_service.dart:137-226` — file transfer has no resume, no integrity check, and reuses the search port; a dropped connection leaves a partial file. Severity: degrades experience.
- `lib/main.dart` — imports `network_discovery_service.dart` twice (once via package URI, once relative). Harmless but indicates churn. Severity: cosmetic.
- Dead/duplicate code: `ui/screens/search_overlay.dart`, `ui/screens/simple_test.dart`, and `depricated/enhanced_search_bar.dart` are unused; `uuid` is a declared dependency with no usage found. Severity: cosmetic.
- `test/widget_test.dart` — the default Flutter counter test referencing `MyApp` and `find.text('0')`; `MyApp` does not exist in this codebase, so the test is stale and would not compile/pass. Severity: cosmetic (no real test coverage). Per audit scope, test contents are listed but not audited.
- README "Smart Commands": `/` folder search is implemented (passes `directoriesOnly`); `@device`/`@settings` are implemented; `calc` keyword is not a feature, though bare arithmetic is auto-evaluated. Google Sign-In is present in UI but documented and coded as non-functional in the desktop window. Severity: degrades experience / cosmetic.

Platform testing: README marks Windows as the supported/tested platform and Linux and macOS as "In Dev"/"Coming Soon." The code paths handle all three desktop platforms (`Platform.isWindows/isLinux/isMacOS` branches in window setup, default directories, downloads directory, and file opening), but only Windows is claimed as verified. There are no mobile code paths in the active flow. A stray empty `flutter_01.png` sits at the legacy root; no build artifacts were audited.

## Rewrite Reference Summary

### What to preserve in intent

The core experience is a global-hotkey transparent launcher overlay that does instant ranked local file search, with inline calculator and `@`/`/` command modes, and the ability to retarget a search at another of the user's devices on the same LAN and then pull a file from it peer-to-peer. Preserve: the Ctrl+Space summon-and-dismiss overlay with keyboard navigation; multi-strategy ranked search (exact > prefix > token > fuzzy, with recency and frequency boosts); LAN auto-discovery of the same user's devices with online/offline status; remote search returning ranked results tagged by device; direct P2P file transfer with a progress indicator into a per-device Downloads folder; an account/device model where a user signs in and sees their devices; and the per-device settings (hotkey rebind, display name, device list, indexed directories).

### What to discard

Discard the in-memory-only index that is rebuilt every launch in favor of a persisted index (the SQLite schema already sketches `files`/`applications`/`search_history` but is never written — the rewrite should actually persist and incrementally update). Discard the bloom filter as currently wired (built but never queried) unless it is integrated into the lookup path. Discard the O(n)-per-query full-scan content and fuzzy passes in favor of index-backed candidate generation. Replace the no-resume, unauthenticated, unencrypted TCP transfer and the unverified HMAC discovery with a framed, resumable, authenticated transfer protocol. Drop the dead/duplicate UI (`SearchOverlay`, `SimpleTestScreen`, deprecated search bar) and the stale default widget test. Reconsider SharedPreferences-via-`ConfigManager` (never initialized) in favor of a single settings store.

### DSA status

- Trie: complete and usable as a reference for behavior (lowercased keys, frequency, associated data, autocompletion), but resolve the duplicate-class artifact and note the uncapped subtree walk before porting.
- Bloom Filter: complete and correct (FNV-1a + DJB2 double hashing, optimal sizing), usable as a reference; the rewrite must decide whether to actually use it in the query path.
- LRU Cache: complete and correct O(1) implementation, directly usable as a reference (mind the null-key/value handling).
- Fuzzy Matcher: complete and rich (seven algorithms) and usable as a reference, but the rewrite should choose a small subset (likely Jaro-Winkler for filenames plus Levenshtein) rather than the four-way combined score run over every file.

None of the four need to be reverse-engineered; all four are full implementations. The open question is integration and performance, not correctness.

### Open questions for the Rust implementation

- Wire format details: the TCP request/response framing relies on "first `{` to last `}`" parsing and a single newline between transfer header and body; the rewrite must define explicit length-delimited framing and decide whether to keep JSON or move to a binary/length-prefixed protocol. The `max_results: 50` in requests is currently ignored server-side.
- Trust model: discovery signatures are unverified and the transfer channel is open. The rewrite must decide how devices authenticate to each other (shared user secret, per-device keys, TLS) and how `file_transfer` requests are authorized (currently any absolute path on the host can be served).
- Index persistence and freshness: there is no file-watching or incremental update; the index is full-rescan only, capped inconsistently at 10k/50k files, depth 8, skipping files over 10 MB and a fixed exclude list. The rewrite must define the persistence store, update strategy, and limits explicitly.
- Identity and offline semantics: "online" currently means "seen on the LAN in the last 20 seconds," while Supabase also tracks an `is_online`/heartbeat flag; the rewrite must reconcile LAN presence vs backend presence and decide whether a cloud backend is even retained.
- Platform assumptions: default directories, the downloads path, and file-open commands branch per OS and are only verified on Windows; the rewrite must confirm Linux/macOS behavior. Window management (frameless transparent 1x1 window, multi-window settings, cursor-display targeting) is desktop-specific and tied to Flutter plugins that have no direct Rust equivalent.
- Content extraction: the indexer reads up to 10 KB of text from a fixed list of text extensions into `metadata['content']` for content search; the rewrite must decide whether to keep content indexing and how to bound it.
