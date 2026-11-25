import { invoke } from "@tauri-apps/api/core";

export type Direction = "rtl" | "ltr";
export type LibraryKind = "local" | "sftp";

export interface Library {
  id: number;
  kind: LibraryKind;
  name: string;
  host: string | null;
  root: string;
  direction: Direction | null;
}

export interface WorkView {
  id: string;
  title: string;
  volumeCount: number;
  readVolumes: number;
  coverUrl: string;
  direction: Direction;
  folderDirection: Direction | null;
}

export interface VolumeView {
  key: string;
  title: string;
  pageCount: number;
  page: number | null;
  coverUrl: string;
  direction: Direction;
}

export interface RecentView {
  libraryId: number;
  key: string;
  title: string;
  workId: string;
  workTitle: string;
  page: number;
  pageCount: number;
  coverUrl: string;
  direction: Direction;
}

export interface OpenedVolume {
  key: string;
  title: string;
  workId: string;
  workTitle: string;
  pages: string[];
  page: number;
  shift: boolean;
  direction: Direction;
  inheritedDirection: Direction;
  prevKey: string | null;
  nextKey: string | null;
  remote: boolean;
}

export interface LibraryUpdated {
  libraryId: number;
  error: string | null;
}

export interface RemoteDir {
  path: string;
  folders: string[];
  zipCount: number;
}

export interface SshPrompt {
  id: number;
  kind: "hostKey" | "passphrase" | "password";
  host: string;
  fingerprint: string | null;
  keyPath: string | null;
}

export type DirectionScope = "global" | "library" | "folder" | "book";

export const api = {
  listLibraries: () => invoke<Library[]>("list_libraries"),
  addLocalLibrary: (path: string) => invoke<number>("add_local_library", { path }),
  removeLibrary: (id: number) => invoke<void>("remove_library", { id }),
  listWorks: (libraryId: number) => invoke<WorkView[]>("list_works", { libraryId }),
  refreshLibrary: (libraryId: number) => invoke<void>("refresh_library", { libraryId }),
  listVolumes: (libraryId: number, workId: string) => invoke<VolumeView[]>("list_volumes", { libraryId, workId }),
  listRecent: () => invoke<RecentView[]>("list_recent"),
  openVolume: (libraryId: number, key: string) => invoke<OpenedVolume>("open_volume", { libraryId, key }),
  saveProgress: (libraryId: number, key: string, page: number, pageCount: number) =>
    invoke<void>("save_progress", { libraryId, key, page, pageCount }),
  setShift: (libraryId: number, key: string, shift: boolean) => invoke<void>("set_shift", { libraryId, key, shift }),
  setDirection: (scope: DirectionScope, libraryId: number | null, target: string | null, direction: Direction | null) =>
    invoke<void>("set_direction", { scope, libraryId, target, direction }),
  globalDirection: () => invoke<Direction>("global_direction"),
  listSshHosts: () => invoke<string[]>("list_ssh_hosts"),
  sftpListDir: (host: string, path: string | null) => invoke<RemoteDir>("sftp_list_dir", { host, path }),
  addSftpLibrary: (host: string, path: string) => invoke<number>("add_sftp_library", { host, path }),
  reconnectLibrary: (libraryId: number) => invoke<void>("reconnect_library", { libraryId }),
  sshPromptReply: (id: number, accept: boolean, secret: string | null, remember: boolean) =>
    invoke<void>("ssh_prompt_reply", { id, accept, secret, remember }),
};
