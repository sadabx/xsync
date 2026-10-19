<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import {
    Folder,
    FolderPlus,
    FolderOpen,
    Cloud,
    GitCompare,
    Clock,
    Activity,
    Settings,
    Search,
    Bell,
    LayoutGrid,
    List,
    ExternalLink,
    Check,
    CircleCheck,
    CircleAlert,
    File,
    FileText,
    FileSpreadsheet,
    Image,
    Video,
    FileCode,
    Archive,
    RefreshCw,
    Play,
    Pause,
    Trash,
    Plus,
    Moon,
    Sun,
    User,
    ChevronRight,
    ArrowUpRight,
    HardDrive,
    ShieldCheck,
    X,
    Ellipsis,
    Link2,
    Laptop,
    Users,
    Star,
    RotateCcw,
  } from '@lucide/svelte';

  interface WatchedDirDto {
    local_path: string;
    drive_folder_id: string;
    drive_folder_name: string;
  }

  interface RecentFileDto {
    rel_path: string;
    size_str: string;
    time_str: string;
  }

  interface DetailedFileDto {
    name: string;
    rel_path: string;
    full_path: string;
    local_root: string;
    folder_name: string;
    size_bytes: number;
    size_str: string;
    modified_secs: number;
    time_str: string;
    synced_time_str: string;
    extension: string;
    category: string;
  }

  interface CategoryStatsDto {
    documents_bytes: number;
    documents_count: number;
    images_bytes: number;
    images_count: number;
    videos_bytes: number;
    videos_count: number;
    audio_bytes: number;
    audio_count: number;
    code_bytes: number;
    code_count: number;
    archives_bytes: number;
    archives_count: number;
    others_bytes: number;
    others_count: number;
  }

  interface DiffItemDto {
    path: string;
    side: string;
  }

  interface AppStateDto {
    is_connected: boolean;
    is_syncing: boolean;
    is_paused: boolean;
    user_name: string;
    user_email: string;
    status_message: string;
    storage_used_str: string;
    storage_total_str: string;
    storage_ratio: number;
    total_directories: number;
    total_files_tracked: number;
    total_bytes_str: string;
    directories: WatchedDirDto[];
  }

  // Navigation: 0 = My Drive (Files), 1 = Computers (Folders), 2 = Google Drive (Auth/Cloud), 3 = Diff & Merge, 4 = Recent Activity, 5 = Settings
  let activeTab = $state<number>(0);
  let isDarkMode = $state<boolean>(false);
  let viewMode = $state<'list' | 'grid'>('list');

  // Search & Filtering
  let searchQuery = $state<string>('');
  let selectedCategory = $state<string>('all'); // 'all', 'document', 'sheet', 'image', 'video', 'code', 'archive'

  // App State
  let state = $state<AppStateDto>({
    is_connected: false,
    is_syncing: false,
    is_paused: false,
    user_name: 'Sadab',
    user_email: '',
    status_message: 'Connecting...',
    storage_used_str: '0 GB',
    storage_total_str: '15 GB',
    storage_ratio: 0,
    total_directories: 0,
    total_files_tracked: 0,
    total_bytes_str: '0 MB',
    directories: [],
  });

  let allFiles = $state<DetailedFileDto[]>([]);
  let recentActivity = $state<RecentFileDto[]>([]);
  let categoryStats = $state<CategoryStatsDto>({
    documents_bytes: 0,
    documents_count: 0,
    images_bytes: 0,
    images_count: 0,
    videos_bytes: 0,
    videos_count: 0,
    audio_bytes: 0,
    audio_count: 0,
    code_bytes: 0,
    code_count: 0,
    archives_bytes: 0,
    archives_count: 0,
    others_bytes: 0,
    others_count: 0,
  });

  // Add folder state
  let newLocalPath = $state<string>('');
  let newRemoteName = $state<string>('');
  let isAddingFolder = $state<boolean>(false);
  let showAddFolderModal = $state<boolean>(false);

  // Diff state
  let diffFolder1 = $state<string>('root');
  let diffFolder2 = $state<string>('');
  let diffResults = $state<DiffItemDto[]>([]);
  let isDiffing = $state<boolean>(false);
  let isMerging = $state<boolean>(false);
  let diffMessage = $state<string>('');

  // Toast
  let toastMsg = $state<string | null>(null);
  let toastType = $state<'info' | 'success' | 'error'>('info');
  let toastTimeout: any = null;

  function showToast(msg: string, type: 'info' | 'success' | 'error' = 'info') {
    if (toastTimeout) clearTimeout(toastTimeout);
    toastMsg = msg;
    toastType = type;
    toastTimeout = setTimeout(() => {
      toastMsg = null;
    }, 3500);
  }

  function toggleTheme() {
    isDarkMode = !isDarkMode;
    if (isDarkMode) {
      document.documentElement.classList.add('dark');
      localStorage.setItem('xsync_theme', 'dark');
    } else {
      document.documentElement.classList.remove('dark');
      localStorage.setItem('xsync_theme', 'light');
    }
  }

  // Background polling state & virtual pagination
  let isFetchingFiles = false;
  let visibleLimit = $state<number>(60);

  $effect(() => {
    // Reset visible count on search or category filter change for instantaneous rendering
    searchQuery;
    selectedCategory;
    visibleLimit = 60;
  });

  async function refreshAppStateOnly() {
    try {
      const prevSyncing = state.is_syncing;
      const s = await invoke<AppStateDto>('get_app_state');
      state = s;
      // If a sync operation just completed, refresh files and category stats once
      if (prevSyncing && !s.is_syncing) {
        refreshFilesAndStats();
      }
    } catch (e: any) {
      console.error('Failed to get app state:', e);
    }
  }

  async function refreshFilesAndStats() {
    if (isFetchingFiles) return;
    isFetchingFiles = true;
    try {
      const [files, stats, recent] = await Promise.all([
        invoke<DetailedFileDto[]>('get_all_files', { limit: 300 }),
        invoke<CategoryStatsDto>('get_category_stats'),
        invoke<RecentFileDto[]>('get_recent_activity', { limit: 50 }),
      ]);
      allFiles = files;
      categoryStats = stats;
      recentActivity = recent;
    } catch (e: any) {
      console.error('Failed to get files:', e);
    } finally {
      isFetchingFiles = false;
    }
  }

  async function refreshAll() {
    await refreshAppStateOnly();
    await refreshFilesAndStats();
  }

  async function handleSyncAll() {
    try {
      const res = await invoke<string>('sync_all');
      showToast(res, 'info');
      await refreshAll();
    } catch (e: any) {
      showToast(String(e), 'error');
    }
  }

  async function handleTogglePause() {
    try {
      const isPaused = await invoke<boolean>('toggle_pause');
      showToast(isPaused ? 'Sync paused' : 'Sync resumed', 'info');
      await refreshAppStateOnly();
    } catch (e: any) {
      showToast(String(e), 'error');
    }
  }

  async function handleBrowseFolder() {
    try {
      const folder = await invoke<string | null>('browse_folder');
      if (folder) {
        newLocalPath = folder;
        if (!newRemoteName.trim()) {
          const parts = folder.split(/[/\\]/).filter(Boolean);
          newRemoteName = parts[parts.length - 1] || 'SyncedFolder';
        }
      }
    } catch (e: any) {
      showToast('Error opening folder dialog: ' + e, 'error');
    }
  }

  async function handleAddDirectory() {
    if (!newLocalPath.trim()) {
      showToast('Please select a local folder', 'error');
      return;
    }
    isAddingFolder = true;
    try {
      const res = await invoke<string>('add_directory', {
        localPath: newLocalPath.trim(),
        remoteName: newRemoteName.trim(),
      });
      showToast(res, 'success');
      newLocalPath = '';
      newRemoteName = '';
      showAddFolderModal = false;
      await refreshAll();
    } catch (e: any) {
      showToast(String(e), 'error');
    } finally {
      isAddingFolder = false;
    }
  }

  async function handleRemoveDirectory(localPath: string) {
    if (!confirm(`Stop syncing this folder:\n${localPath}?`)) {
      return;
    }
    try {
      const res = await invoke<string>('remove_directory', { localPath });
      showToast(res, 'info');
      await refreshAll();
    } catch (e: any) {
      showToast(String(e), 'error');
    }
  }

  async function handleOpenFolder(path: string) {
    try {
      await invoke('open_folder', { path });
    } catch (e: any) {
      showToast('Failed to open file manager: ' + e, 'error');
    }
  }

  function handleCopyPath(path: string) {
    navigator.clipboard.writeText(path);
    showToast('Path copied to clipboard: ' + path, 'info');
  }

  async function handleRunDiff() {
    if (!diffFolder2.trim()) {
      showToast('Please enter target folder name or ID', 'error');
      return;
    }
    isDiffing = true;
    diffMessage = '';
    diffResults = [];
    try {
      const res = await invoke<DiffItemDto[]>('run_diff', {
        folder1: diffFolder1.trim(),
        folder2: diffFolder2.trim(),
      });
      diffResults = res;
      if (res.length === 0) {
        diffMessage = 'Folders are identical! No differences found.';
      }
    } catch (e: any) {
      showToast(String(e), 'error');
    } finally {
      isDiffing = false;
    }
  }

  async function handleRunMerge() {
    if (!diffFolder2.trim()) {
      showToast('Please enter destination folder name or ID', 'error');
      return;
    }
    isMerging = true;
    try {
      const res = await invoke<string>('run_merge', {
        source: diffFolder1.trim(),
        destination: diffFolder2.trim(),
      });
      showToast(res, 'success');
      diffMessage = res;
      await handleRunDiff();
    } catch (e: any) {
      showToast(String(e), 'error');
    } finally {
      isMerging = false;
    }
  }

  async function handleSystemd(action: string) {
    try {
      const res = await invoke<string>('systemd_action', { action });
      showToast(res, 'success');
    } catch (e: any) {
      showToast(String(e), 'error');
    }
  }

  async function handleLogin() {
    showToast('Opening Google OAuth in browser...', 'info');
    try {
      const res = await invoke<string>('login');
      showToast(res, 'success');
      await refreshAll();
    } catch (e: any) {
      showToast(String(e), 'error');
    }
  }

  async function handleLogout() {
    if (!confirm('Are you sure you want to sign out and disconnect Google Drive?')) {
      return;
    }
    try {
      const res = await invoke<string>('logout');
      showToast(res, 'info');
      await refreshAll();
    } catch (e: any) {
      showToast(String(e), 'error');
    }
  }

  // Filtered files based on search & category
  let filteredFiles = $derived(
    allFiles.filter((f) => {
      let matchesCat = true;
      if (selectedCategory === 'document') {
        matchesCat = f.category === 'document' && !['xls', 'xlsx', 'csv'].includes(f.extension);
      } else if (selectedCategory === 'sheet') {
        matchesCat = ['xls', 'xlsx', 'csv'].includes(f.extension);
      } else if (selectedCategory === 'image') {
        matchesCat = f.category === 'image';
      } else if (selectedCategory === 'video') {
        matchesCat = f.category === 'video';
      } else if (selectedCategory === 'code') {
        matchesCat = f.category === 'code';
      } else if (selectedCategory === 'archive') {
        matchesCat = f.category === 'archive';
      }

      const q = searchQuery.toLowerCase().trim();
      const matchesSearch =
        !q ||
        f.name.toLowerCase().includes(q) ||
        f.rel_path.toLowerCase().includes(q) ||
        f.folder_name.toLowerCase().includes(q) ||
        f.extension.toLowerCase().includes(q);
      return matchesCat && matchesSearch;
    })
  );

  onMount(() => {
    const saved = localStorage.getItem('xsync_theme');
    if (saved === 'dark') {
      isDarkMode = true;
      document.documentElement.classList.add('dark');
    } else {
      isDarkMode = false;
      document.documentElement.classList.remove('dark');
    }

    refreshAll();

    // Fast 3-second heartbeat checking ONLY daemon status & active sync state
    const interval = setInterval(() => {
      refreshAppStateOnly();
    }, 3000);

    return () => clearInterval(interval);
  });
</script>

<div class="flex h-screen w-screen overflow-hidden bg-[#f8fafd] text-slate-800 dark:bg-[#131314] dark:text-[#e3e3e3] font-sans select-none antialiased">
  <!-- ======================== LEFT SIDEBAR (GOOGLE DRIVE STYLE) ======================== -->
  <aside class="w-[230px] flex-shrink-0 flex flex-col justify-between bg-[#f8fafd] dark:bg-[#131314] text-slate-800 dark:text-[#c4c7c5] p-3 transition-colors z-20">
    <div class="flex flex-col gap-5">
      <!-- App Brand Logo (No 'DRIVE' badge) -->
      <div class="flex items-center gap-3 px-2 pt-1">
        <div class="relative flex items-center justify-center w-9 h-9 rounded-xl bg-white dark:bg-[#282a2c] text-[#1967d2] dark:text-[#7cacf8] border border-slate-200/60 dark:border-[#444746]/40 shadow-xs flex-shrink-0 font-bold">
          <HardDrive class="w-5 h-5 text-[#1967d2] dark:text-[#7cacf8]" />
          {#if state.is_syncing}
            <span class="absolute -top-1 -right-1 flex h-2.5 w-2.5">
              <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-75"></span>
              <span class="relative inline-flex rounded-full h-2.5 w-2.5 bg-amber-400"></span>
            </span>
          {/if}
        </div>
        <div class="flex flex-col">
          <span class="font-bold text-base tracking-tight text-slate-900 dark:text-[#e3e3e3]">xsync</span>
          <span class="text-[11px] font-medium text-slate-500 dark:text-[#8e918f]">Desktop Edition</span>
        </div>
      </div>

      <!-- Action Button: + Add Sync Folder -->
      <button
        onclick={() => showAddFolderModal = true}
        class="flex items-center justify-center gap-2.5 w-full py-2.5 rounded-full bg-white dark:bg-[#282a2c] hover:bg-slate-50 dark:hover:bg-[#333538] text-slate-800 dark:text-[#e3e3e3] border border-slate-200/80 dark:border-[#444746]/50 font-semibold text-xs shadow-xs transition-colors active:scale-[0.98]"
      >
        <Plus class="w-4 h-4 text-[#1967d2] dark:text-[#7cacf8]" />
        <span>Add Sync Folder</span>
      </button>

      <!-- Navigation Menu Items -->
      <nav class="flex flex-col gap-1">
        <button
          onclick={() => activeTab = 0}
          class="flex items-center gap-3.5 px-3.5 py-2 rounded-full text-xs transition-colors text-left
            {activeTab === 0
              ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-semibold'
              : 'text-slate-700 dark:text-[#c4c7c5] hover:bg-slate-200/60 dark:hover:bg-[#1f1f23] dark:hover:text-[#e3e3e3]'}"
        >
          <Folder class="w-4 h-4 flex-shrink-0" />
          <span>My Drive</span>
          {#if state.total_files_tracked > 0}
            <span class="ml-auto text-[10px] font-bold px-2 py-0.5 rounded-full
              {activeTab === 0
                ? 'bg-[#001d35] text-white dark:bg-[#c2e7ff] dark:text-[#001d35]'
                : 'bg-slate-200 text-slate-700 dark:bg-[#282a2c] dark:text-[#c4c7c5]'}"
            >
              {state.total_files_tracked}
            </span>
          {/if}
        </button>

        <button
          onclick={() => activeTab = 1}
          class="flex items-center gap-3.5 px-3.5 py-2 rounded-full text-xs transition-colors text-left
            {activeTab === 1
              ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-semibold'
              : 'text-slate-700 dark:text-[#c4c7c5] hover:bg-slate-200/60 dark:hover:bg-[#1f1f23] dark:hover:text-[#e3e3e3]'}"
        >
          <Laptop class="w-4 h-4 flex-shrink-0" />
          <span>Computers</span>
          {#if state.total_directories > 0}
            <span class="ml-auto text-[10px] font-bold px-2 py-0.5 rounded-full
              {activeTab === 1
                ? 'bg-[#001d35] text-white dark:bg-[#c2e7ff] dark:text-[#001d35]'
                : 'bg-slate-200 text-slate-700 dark:bg-[#282a2c] dark:text-[#c4c7c5]'}"
            >
              {state.total_directories}
            </span>
          {/if}
        </button>

        <button
          onclick={() => activeTab = 2}
          class="flex items-center gap-3.5 px-3.5 py-2 rounded-full text-xs transition-colors text-left
            {activeTab === 2
              ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-semibold'
              : 'text-slate-700 dark:text-[#c4c7c5] hover:bg-slate-200/60 dark:hover:bg-[#1f1f23] dark:hover:text-[#e3e3e3]'}"
        >
          <Cloud class="w-4 h-4 flex-shrink-0" />
          <span>Google Drive</span>
          {#if state.is_connected}
            <span class="ml-auto flex h-2 w-2 rounded-full bg-emerald-500 dark:bg-[#6dd58c]"></span>
          {/if}
        </button>

        <button
          onclick={() => activeTab = 3}
          class="flex items-center gap-3.5 px-3.5 py-2 rounded-full text-xs transition-colors text-left
            {activeTab === 3
              ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-semibold'
              : 'text-slate-700 dark:text-[#c4c7c5] hover:bg-slate-200/60 dark:hover:bg-[#1f1f23] dark:hover:text-[#e3e3e3]'}"
        >
          <GitCompare class="w-4 h-4 flex-shrink-0" />
          <span>Diff & Merge</span>
        </button>

        <button
          onclick={() => activeTab = 4}
          class="flex items-center gap-3.5 px-3.5 py-2 rounded-full text-xs transition-colors text-left
            {activeTab === 4
              ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-semibold'
              : 'text-slate-700 dark:text-[#c4c7c5] hover:bg-slate-200/60 dark:hover:bg-[#1f1f23] dark:hover:text-[#e3e3e3]'}"
        >
          <Clock class="w-4 h-4 flex-shrink-0" />
          <span>Recent</span>
        </button>

        <button
          onclick={() => activeTab = 5}
          class="flex items-center gap-3.5 px-3.5 py-2 rounded-full text-xs transition-colors text-left
            {activeTab === 5
              ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-semibold'
              : 'text-slate-700 dark:text-[#c4c7c5] hover:bg-slate-200/60 dark:hover:bg-[#1f1f23] dark:hover:text-[#e3e3e3]'}"
        >
          <Settings class="w-4 h-4 flex-shrink-0" />
          <span>Settings</span>
        </button>
      </nav>
    </div>

    <!-- STORAGE DETAILS (Google Drive Material 3 Style) -->
    <div class="flex flex-col gap-3 pt-3 border-t border-slate-200 dark:border-[#2d2f31] text-slate-700 dark:text-[#c4c7c5]">
      <div class="flex items-center gap-2 text-xs font-semibold text-slate-800 dark:text-[#e3e3e3]">
        <Cloud class="w-4 h-4 text-[#1967d2] dark:text-[#7cacf8]" />
        <span>Storage</span>
      </div>

      <!-- Storage Meter: Google Drive Cloud -->
      <div class="flex flex-col gap-1">
        <div class="h-1 w-full rounded-full bg-slate-200 dark:bg-[#282a2c] overflow-hidden">
          <div
            class="h-full rounded-full bg-[#1967d2] dark:bg-[#7cacf8] transition-all duration-300"
            style="width: {Math.max(state.storage_ratio * 100, state.is_connected ? 2 : 0)}%"
          ></div>
        </div>
        <span class="text-[11px] text-slate-500 dark:text-[#8e918f] font-sans">
          {state.storage_used_str} of {state.storage_total_str} used
        </span>
      </div>

      <!-- Local Cache Info -->
      <div class="flex items-center justify-between text-[11px] text-slate-500 dark:text-[#8e918f]">
        <span>Local Cache</span>
        <span class="font-mono">{state.total_bytes_str}</span>
      </div>

      <!-- Upgrade / Manage Storage Link -->
      <button
        onclick={() => activeTab = 2}
        class="flex items-center gap-1.5 text-xs font-medium text-[#1967d2] dark:text-[#7cacf8] hover:underline pt-0.5 text-left"
      >
        <span>Manage Storage</span>
        <ArrowUpRight class="w-3.5 h-3.5" />
      </button>
    </div>
  </aside>

  <!-- ======================== MAIN WORKSPACE (ELEVATED CARD) ======================== -->
  <main class="flex-1 flex flex-col min-w-0 bg-white dark:bg-[#1e1f20] rounded-2xl md:rounded-3xl border border-slate-200/60 dark:border-[#2d2f31]/60 my-2 mr-2 overflow-hidden shadow-xs">
    <!-- TOP HEADER BAR -->
    <header class="h-16 flex-shrink-0 flex items-center justify-between border-b border-slate-100 dark:border-[#2d2f31] px-6 gap-4">
      <!-- Search in Drive Pill -->
      <div class="relative flex-1 max-w-lg">
        <Search class="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400 dark:text-[#8e918f]" />
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Search in Drive..."
          class="w-full pl-11 pr-4 py-2 text-xs rounded-full bg-[#edf2fa] dark:bg-[#282a2c] text-slate-800 dark:text-[#e3e3e3] placeholder:text-slate-400 dark:placeholder:text-[#8e918f] focus:outline-none focus:ring-2 focus:ring-[#1967d2]/40 dark:focus:ring-[#7cacf8]/40 transition-colors"
        />
      </div>

      <!-- Right Header Actions (Status pill, Contextual Sync/Resume, Theme, Avatar) -->
      <div class="flex items-center gap-3">
        <!-- SINGLE UNIFIED SYNC CONTROL ("One for all") -->
        {#if state.is_paused}
          <!-- Paused State: Single Resume Sync button -->
          <button
            onclick={handleTogglePause}
            class="flex items-center gap-1.5 px-4 py-1.5 rounded-full text-xs font-semibold bg-emerald-600 hover:bg-emerald-700 dark:bg-[#1e8e3e] dark:hover:bg-[#25a244] text-white transition-colors shadow-xs active:scale-95 cursor-pointer"
            title="Synchronization is paused. Click to resume."
          >
            <Play class="w-3.5 h-3.5 fill-current" />
            <span>Resume Sync</span>
          </button>
        {:else if state.is_syncing}
          <!-- Syncing State: Single Syncing indicator button -->
          <div
            class="flex items-center gap-2 px-4 py-1.5 rounded-full text-xs font-semibold bg-[#1967d2] dark:bg-[#0b57d0] text-white shadow-xs cursor-default"
            title="Synchronizing files with Google Drive..."
          >
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
            <span>Syncing...</span>
          </div>
        {:else if !state.is_connected}
          <!-- Offline State: Single Offline badge -->
          <div
            class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-full text-xs font-semibold bg-slate-100 dark:bg-[#282a2c] text-slate-500 dark:text-[#8e918f] border border-slate-200 dark:border-[#444746]/40 cursor-default"
          >
            <CircleAlert class="w-3.5 h-3.5" />
            <span>Offline</span>
          </div>
        {:else}
          <!-- Active / Synced State: Single Unified Capsule (Status/Sync Now + Pause) -->
          <div class="flex items-center rounded-full bg-[#edf2fa] dark:bg-[#282a2c] border border-slate-200/80 dark:border-[#444746]/50 shadow-xs overflow-hidden">
            <button
              onclick={handleSyncAll}
              class="group flex items-center gap-2 pl-3.5 pr-2.5 py-1.5 text-xs font-semibold text-slate-800 dark:text-[#e3e3e3] hover:text-[#1967d2] dark:hover:text-[#7cacf8] transition-colors cursor-pointer"
              title="All files up to date. Click to force check now."
            >
              <Check class="w-3.5 h-3.5 text-emerald-600 dark:text-[#6dd58c] group-hover:hidden" />
              <RefreshCw class="w-3.5 h-3.5 text-[#1967d2] dark:text-[#7cacf8] hidden group-hover:inline animate-spin" />
              <span class="group-hover:hidden">All Synced</span>
              <span class="hidden group-hover:inline">Sync Now</span>
            </button>

            <div class="h-3.5 w-px bg-slate-300 dark:bg-[#444746]/70"></div>

            <button
              onclick={handleTogglePause}
              class="pr-3 pl-2 py-1.5 text-slate-400 dark:text-[#8e918f] hover:text-amber-600 dark:hover:text-[#ffddb3] hover:bg-slate-200/50 dark:hover:bg-[#333538] transition-colors cursor-pointer"
              title="Pause automatic synchronization"
            >
              <Pause class="w-3.5 h-3.5" />
            </button>
          </div>
        {/if}

        <!-- Dark/Light Theme Button -->
        <button
          onclick={toggleTheme}
          class="p-2 rounded-full text-slate-500 dark:text-[#c4c7c5] hover:bg-slate-100 dark:hover:bg-[#282a2c] transition-colors"
          title={isDarkMode ? 'Switch to Light Mode' : 'Switch to Dark Mode'}
        >
          {#if isDarkMode}
            <Sun class="w-4 h-4 text-amber-400" />
          {:else}
            <Moon class="w-4 h-4 text-slate-700" />
          {/if}
        </button>

        <div class="h-5 w-px bg-slate-200 dark:bg-[#2d2f31] mx-0.5"></div>

        <!-- User Profile Avatar Bubble -->
        <div class="flex items-center gap-2">
          <span class="text-xs font-semibold text-slate-700 dark:text-[#e3e3e3] hidden md:inline">
            {state.user_name || 'Sadab'}
          </span>
          <div class="w-8 h-8 rounded-full flex items-center justify-center bg-[#1967d2] dark:bg-[#0b57d0] text-white font-bold text-xs shadow-xs">
            {state.user_name ? state.user_name.charAt(0).toUpperCase() : 'S'}
          </div>
        </div>
      </div>
    </header>

    <!-- CONTENT BODY (SCROLLABLE) -->
    <div class="flex-1 overflow-y-auto px-8 py-6">
      <!-- TOAST NOTIFICATION -->
      {#if toastMsg}
        <div class="mb-5 flex items-center justify-between px-4 py-3 rounded-xl text-xs font-semibold shadow-md transition-all
          {toastType === 'success'
            ? 'bg-emerald-50 dark:bg-[#1a2e22] border border-emerald-300 dark:border-[#2b4c37] text-emerald-800 dark:text-[#6dd58c]'
            : toastType === 'error'
            ? 'bg-rose-50 dark:bg-[#3b1d24] border border-rose-300 dark:border-[#632733] text-rose-800 dark:text-[#f87171]'
            : 'bg-blue-50 dark:bg-[#004a77] border border-blue-300 dark:border-[#005c91] text-blue-800 dark:text-[#c2e7ff]'}"
        >
          <div class="flex items-center gap-2.5">
            {#if toastType === 'success'}
              <CircleCheck class="w-4 h-4 text-emerald-500" />
            {:else if toastType === 'error'}
              <CircleAlert class="w-4 h-4 text-rose-500" />
            {:else}
              <Activity class="w-4 h-4 text-blue-500" />
            {/if}
            <span>{toastMsg}</span>
          </div>
          <button onclick={() => toastMsg = null} class="p-1 hover:opacity-75">
            <X class="w-3.5 h-3.5" />
          </button>
        </div>
      {/if}

      <!-- ==================== TAB 0: MY DRIVE (FILES EXPLORER) ==================== -->
      {#if activeTab === 0}
        <div class="flex flex-col gap-6 max-w-6xl mx-auto">
          <!-- View Title Header with Folder Badge and View Toggles -->
          <div class="flex items-center justify-between pb-1 border-b border-slate-100 dark:border-[#2d2f31]">
            <div class="flex items-center gap-3">
              <h1 class="text-2xl font-bold tracking-tight text-slate-800 dark:text-[#e3e3e3]">
                My Drive
              </h1>
              <div class="p-1.5 rounded-lg bg-[#edf2fa] dark:bg-[#282a2c] text-[#1967d2] dark:text-[#7cacf8]">
                <Folder class="w-4 h-4" />
              </div>
            </div>

            <!-- Category Filter Pills & List/Grid Switcher -->
            <div class="flex items-center gap-3">
              <!-- Filter category pills -->
              <div class="flex items-center gap-1.5">
                <button
                  onclick={() => selectedCategory = 'all'}
                  class="px-3.5 py-1.5 rounded-full text-xs transition-colors
                    {selectedCategory === 'all'
                      ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-medium'
                      : 'bg-[#edf2fa] text-slate-600 dark:bg-[#282a2c] dark:text-[#c4c7c5] hover:bg-slate-200/70 dark:hover:bg-[#333538] border border-transparent dark:border-[#444746]/50'}"
                >
                  All ({allFiles.length})
                </button>
                <button
                  onclick={() => selectedCategory = 'document'}
                  class="px-3.5 py-1.5 rounded-full text-xs transition-colors
                    {selectedCategory === 'document'
                      ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-medium'
                      : 'bg-[#edf2fa] text-slate-600 dark:bg-[#282a2c] dark:text-[#c4c7c5] hover:bg-slate-200/70 dark:hover:bg-[#333538] border border-transparent dark:border-[#444746]/50'}"
                >
                  Documents
                </button>
                <button
                  onclick={() => selectedCategory = 'sheet'}
                  class="px-3.5 py-1.5 rounded-full text-xs transition-colors
                    {selectedCategory === 'sheet'
                      ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-medium'
                      : 'bg-[#edf2fa] text-slate-600 dark:bg-[#282a2c] dark:text-[#c4c7c5] hover:bg-slate-200/70 dark:hover:bg-[#333538] border border-transparent dark:border-[#444746]/50'}"
                >
                  Sheets
                </button>
                <button
                  onclick={() => selectedCategory = 'image'}
                  class="px-3.5 py-1.5 rounded-full text-xs transition-colors
                    {selectedCategory === 'image'
                      ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-medium'
                      : 'bg-[#edf2fa] text-slate-600 dark:bg-[#282a2c] dark:text-[#c4c7c5] hover:bg-slate-200/70 dark:hover:bg-[#333538] border border-transparent dark:border-[#444746]/50'}"
                >
                  Photos
                </button>
                <button
                  onclick={() => selectedCategory = 'video'}
                  class="px-3.5 py-1.5 rounded-full text-xs transition-colors
                    {selectedCategory === 'video'
                      ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-medium'
                      : 'bg-[#edf2fa] text-slate-600 dark:bg-[#282a2c] dark:text-[#c4c7c5] hover:bg-slate-200/70 dark:hover:bg-[#333538] border border-transparent dark:border-[#444746]/50'}"
                >
                  Videos
                </button>
                <button
                  onclick={() => selectedCategory = 'code'}
                  class="px-3.5 py-1.5 rounded-full text-xs transition-colors
                    {selectedCategory === 'code'
                      ? 'bg-[#c2e7ff] text-[#001d35] dark:bg-[#004a77] dark:text-[#c2e7ff] font-medium'
                      : 'bg-[#edf2fa] text-slate-600 dark:bg-[#282a2c] dark:text-[#c4c7c5] hover:bg-slate-200/70 dark:hover:bg-[#333538] border border-transparent dark:border-[#444746]/50'}"
                >
                  Code
                </button>
              </div>

              <!-- Grid / List View Toggle -->
              <div class="flex items-center gap-0.5 bg-[#edf2fa] dark:bg-[#282a2c] p-0.5 rounded-full border border-slate-200/60 dark:border-[#444746]/50">
                <button
                  onclick={() => viewMode = 'list'}
                  class="p-1.5 rounded-full transition-colors {viewMode === 'list' ? 'bg-white dark:bg-[#004a77] text-[#1967d2] dark:text-[#c2e7ff] shadow-xs' : 'text-slate-400 dark:text-[#8e918f] hover:text-slate-700 dark:hover:text-[#e3e3e3]'}"
                  title="List view"
                >
                  <List class="w-3.5 h-3.5" />
                </button>
                <button
                  onclick={() => viewMode = 'grid'}
                  class="p-1.5 rounded-full transition-colors {viewMode === 'grid' ? 'bg-white dark:bg-[#004a77] text-[#1967d2] dark:text-[#c2e7ff] shadow-xs' : 'text-slate-400 dark:text-[#8e918f] hover:text-slate-700 dark:hover:text-[#e3e3e3]'}"
                  title="Grid view"
                >
                  <LayoutGrid class="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          </div>

          <!-- ALL FILES TABLE SECTION -->
          <div class="flex flex-col">
            <span class="text-[11px] font-medium text-slate-400 dark:text-[#8e918f] tracking-wider uppercase mb-2">
              All Files
            </span>

            {#if filteredFiles.length === 0}
              <div class="py-20 flex flex-col items-center justify-center text-center">
                <div class="p-4 rounded-full bg-slate-100 dark:bg-[#282a2c] text-slate-400 dark:text-[#8e918f] mb-3">
                  <Search class="w-8 h-8" />
                </div>
                <p class="text-sm font-bold text-slate-700 dark:text-[#e3e3e3]">No files found</p>
                <p class="text-xs text-slate-400 dark:text-[#8e918f] mt-1">Try searching for a different keyword or category.</p>
              </div>
            {:else if viewMode === 'list'}
              <!-- LIST VIEW TABLE -->
              <div class="w-full overflow-hidden">
                <!-- Table Header -->
                <div class="grid grid-cols-12 px-4 py-2 text-[11px] font-medium text-slate-400 dark:text-[#8e918f] border-b border-slate-100 dark:border-[#2d2f31]">
                  <div class="col-span-5">Name</div>
                  <div class="col-span-2">Owner</div>
                  <div class="col-span-3">Last Modified</div>
                  <div class="col-span-1 text-right">File Size</div>
                  <div class="col-span-1 text-right">Actions</div>
                </div>

                <!-- Table Rows (Google Drive Style Row Hover) -->
                <div class="divide-y divide-slate-100 dark:divide-[#282a2c]">
                  {#each filteredFiles.slice(0, visibleLimit) as f}
                    <div
                      onclick={() => handleOpenFolder(f.full_path)}
                      class="grid grid-cols-12 items-center px-4 py-3 text-xs cursor-pointer group hover:bg-[#e8f0fe] dark:hover:bg-[#282a2c] transition-colors"
                    >
                      <!-- NAME COLUMN WITH GOOGLE DRIVE FILE TYPE ICONS -->
                      <div class="col-span-5 flex items-center gap-3.5 min-w-0 pr-2">
                        {#if f.extension === 'pdf'}
                          <div class="w-7 h-7 rounded-md bg-[#ea4335] text-white flex items-center justify-center flex-shrink-0 shadow-xs">
                            <FileText class="w-4 h-4" />
                          </div>
                        {:else if ['xls', 'xlsx', 'csv'].includes(f.extension)}
                          <div class="w-7 h-7 rounded-md bg-[#1e8e3e] text-white flex items-center justify-center flex-shrink-0 shadow-xs">
                            <FileSpreadsheet class="w-4 h-4" />
                          </div>
                        {:else if ['ppt', 'pptx'].includes(f.extension)}
                          <div class="w-7 h-7 rounded-md bg-[#f29900] text-white flex items-center justify-center flex-shrink-0 shadow-xs">
                            <FileText class="w-4 h-4" />
                          </div>
                        {:else if f.category === 'image'}
                          <div class="w-7 h-7 rounded-md bg-[#fa7b17] text-white flex items-center justify-center flex-shrink-0 shadow-xs">
                            <Image class="w-4 h-4" />
                          </div>
                        {:else if f.category === 'video'}
                          <div class="w-7 h-7 rounded-md bg-[#e0284f] text-white flex items-center justify-center flex-shrink-0 shadow-xs">
                            <Video class="w-4 h-4" />
                          </div>
                        {:else if f.category === 'code'}
                          <div class="w-7 h-7 rounded-md bg-[#0284c7] text-white flex items-center justify-center flex-shrink-0 shadow-xs">
                            <FileCode class="w-4 h-4" />
                          </div>
                        {:else if f.category === 'archive'}
                          <div class="w-7 h-7 rounded-md bg-[#64748b] text-white flex items-center justify-center flex-shrink-0 shadow-xs">
                            <Archive class="w-4 h-4" />
                          </div>
                        {:else}
                          <div class="w-7 h-7 rounded-md bg-[#1a73e8] text-white flex items-center justify-center flex-shrink-0 shadow-xs">
                            <FileText class="w-4 h-4" />
                          </div>
                        {/if}

                        <span class="font-medium text-slate-800 dark:text-[#e3e3e3] truncate group-hover:text-[#1967d2] dark:group-hover:text-[#7cacf8]">
                          {f.name}
                        </span>
                      </div>

                      <!-- OWNER COLUMN -->
                      <div class="col-span-2 flex items-center gap-1.5">
                        <div class="w-6 h-6 rounded-full bg-amber-500 text-white font-bold text-[10px] flex items-center justify-center shadow-xs">
                          {state.user_name ? state.user_name.charAt(0).toUpperCase() : 'S'}
                        </div>
                        <span class="text-xs text-slate-500 dark:text-[#8e918f] truncate">
                          {state.user_name || 'Sadab'}
                        </span>
                      </div>

                      <!-- LAST MODIFIED COLUMN -->
                      <div class="col-span-3 text-xs text-slate-500 dark:text-[#8e918f] truncate">
                        {f.time_str}
                      </div>

                      <!-- FILE SIZE COLUMN -->
                      <div class="col-span-1 text-xs font-medium text-slate-700 dark:text-[#c4c7c5] text-right">
                        {f.size_str}
                      </div>

                      <!-- ACTIONS COLUMN -->
                      <div class="col-span-1 flex items-center justify-end gap-1" onclick={(e) => e.stopPropagation()}>
                        <button
                          onclick={() => handleCopyPath(f.full_path)}
                          class="p-1.5 rounded-md text-slate-400 dark:text-[#8e918f] hover:text-[#1967d2] dark:hover:text-[#c2e7ff] hover:bg-slate-100 dark:hover:bg-[#333538] transition-colors"
                          title="Copy file path"
                        >
                          <Link2 class="w-3.5 h-3.5" />
                        </button>
                        <button
                          onclick={() => handleOpenFolder(f.full_path)}
                          class="p-1.5 rounded-md text-slate-400 dark:text-[#8e918f] hover:text-[#1967d2] dark:hover:text-[#c2e7ff] hover:bg-slate-100 dark:hover:bg-[#333538] transition-colors"
                          title="Reveal in file manager"
                        >
                          <FolderOpen class="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {:else}
              <!-- GRID VIEW CARDS -->
              <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                {#each filteredFiles.slice(0, visibleLimit) as f}
                  <div
                    onclick={() => handleOpenFolder(f.full_path)}
                    class="flex flex-col justify-between p-4 rounded-2xl border border-slate-200/80 dark:border-[#444746]/40 bg-[#f8fafc] dark:bg-[#282a2c] hover:bg-[#e8f0fe] dark:hover:bg-[#333538] hover:border-blue-400/50 transition-colors cursor-pointer group"
                  >
                    <div class="flex items-start justify-between">
                      {#if f.extension === 'pdf'}
                        <div class="w-9 h-9 rounded-lg bg-[#ea4335] text-white flex items-center justify-center shadow-xs">
                          <FileText class="w-5 h-5" />
                        </div>
                      {:else if ['xls', 'xlsx', 'csv'].includes(f.extension)}
                        <div class="w-9 h-9 rounded-lg bg-[#1e8e3e] text-white flex items-center justify-center shadow-xs">
                          <FileSpreadsheet class="w-5 h-5" />
                        </div>
                      {:else if f.category === 'image'}
                        <div class="w-9 h-9 rounded-lg bg-[#fa7b17] text-white flex items-center justify-center shadow-xs">
                          <Image class="w-5 h-5" />
                        </div>
                      {:else if f.category === 'video'}
                        <div class="w-9 h-9 rounded-lg bg-[#e0284f] text-white flex items-center justify-center shadow-xs">
                          <Video class="w-5 h-5" />
                        </div>
                      {:else if f.category === 'code'}
                        <div class="w-9 h-9 rounded-lg bg-[#0284c7] text-white flex items-center justify-center shadow-xs">
                          <FileCode class="w-5 h-5" />
                        </div>
                      {:else}
                        <div class="w-9 h-9 rounded-lg bg-[#1a73e8] text-white flex items-center justify-center shadow-xs">
                          <FileText class="w-5 h-5" />
                        </div>
                      {/if}

                      <button
                        onclick={(e) => { e.stopPropagation(); handleOpenFolder(f.full_path); }}
                        class="p-1 rounded text-slate-400 dark:text-[#8e918f] hover:text-[#1967d2] dark:hover:text-[#c2e7ff]"
                        title="Open folder"
                      >
                        <FolderOpen class="w-4 h-4" />
                      </button>
                    </div>

                    <div class="flex flex-col mt-3">
                      <span class="text-xs font-semibold text-slate-800 dark:text-[#e3e3e3] truncate group-hover:text-[#1967d2] dark:group-hover:text-[#7cacf8]">
                        {f.name}
                      </span>
                      <span class="text-[11px] text-slate-500 dark:text-[#8e918f] mt-0.5">
                        {f.size_str} • {f.folder_name}
                      </span>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}

            <!-- Virtual Windowing: Show More Button -->
            {#if filteredFiles.length > visibleLimit}
              <div class="py-5 text-center">
                <button
                  onclick={() => visibleLimit += 60}
                  class="px-5 py-2 rounded-full text-xs font-medium bg-slate-100 hover:bg-slate-200 text-slate-700 dark:bg-[#282a2c] dark:hover:bg-[#333538] dark:text-[#c4c7c5] dark:hover:text-[#e3e3e3] border border-slate-200 dark:border-[#444746]/50 transition-colors shadow-xs"
                >
                  Show more ({filteredFiles.length - visibleLimit} remaining)
                </button>
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <!-- ==================== TAB 1: COMPUTERS (SYNCED FOLDERS) ==================== -->
      {#if activeTab === 1}
        <div class="flex flex-col gap-6 max-w-4xl mx-auto">
          <div class="flex items-center justify-between pb-1 border-b border-slate-100 dark:border-[#2d2f31]">
            <div class="flex flex-col">
              <h1 class="text-2xl font-bold tracking-tight text-slate-800 dark:text-[#e3e3e3]">Computers & Synced Folders</h1>
              <p class="text-xs text-slate-500 dark:text-[#8e918f] mt-1">Manage local folders synchronized bidirectionally with Google Drive.</p>
            </div>
            <button
              onclick={() => showAddFolderModal = true}
              class="flex items-center gap-1.5 px-4 py-2 rounded-full bg-[#1967d2] hover:bg-[#1557bf] dark:bg-[#0b57d0] dark:hover:bg-[#1b6ef3] text-white text-xs font-semibold shadow-xs transition-colors"
            >
              <Plus class="w-4 h-4" />
              <span>Link Folder</span>
            </button>
          </div>

          <!-- Active Folders List -->
          <div class="flex flex-col rounded-2xl border border-slate-200/80 dark:border-[#444746]/40 bg-white dark:bg-[#282a2c] shadow-xs overflow-hidden">
            <div class="px-5 py-3.5 border-b border-slate-100 dark:border-[#2d2f31] flex items-center justify-between">
              <span class="text-xs font-semibold uppercase text-slate-400 dark:text-[#8e918f]">Configured Directories</span>
              <span class="text-xs font-medium text-slate-500 dark:text-[#c4c7c5]">{state.directories.length} folders</span>
            </div>

            <div class="divide-y divide-slate-100 dark:divide-[#2d2f31]">
              {#each state.directories as dir}
                <div class="p-4 flex items-center justify-between hover:bg-[#f8fafc] dark:hover:bg-[#333538] transition-colors">
                  <div class="flex items-center gap-3.5 min-w-0">
                    <div class="w-10 h-10 rounded-xl bg-[#edf2fa] dark:bg-[#1e1f20] text-[#1967d2] dark:text-[#7cacf8] flex items-center justify-center flex-shrink-0">
                      <Folder class="w-5 h-5" />
                    </div>
                    <div class="flex flex-col min-w-0">
                      <span class="text-sm font-semibold text-slate-900 dark:text-[#e3e3e3] truncate">
                        {dir.local_path.split(/[/\\]/).filter(Boolean).pop() || dir.drive_folder_name}
                      </span>
                      <span class="text-xs font-mono text-slate-400 dark:text-[#8e918f] truncate">
                        {dir.local_path}
                      </span>
                    </div>
                  </div>

                  <div class="flex items-center gap-3">
                    <span class="text-[11px] font-mono px-2.5 py-1 rounded-md bg-slate-100 dark:bg-[#1e1f20] text-slate-600 dark:text-[#c4c7c5] border border-slate-200 dark:border-[#444746]/50">
                      ID: {dir.drive_folder_id.slice(0, 10)}...
                    </span>
                    <button
                      onclick={() => handleOpenFolder(dir.local_path)}
                      class="p-2 rounded-lg text-slate-400 dark:text-[#8e918f] hover:text-[#1967d2] dark:hover:text-[#c2e7ff] hover:bg-slate-100 dark:hover:bg-[#1e1f20] transition-colors"
                      title="Reveal in file manager"
                    >
                      <FolderOpen class="w-4 h-4" />
                    </button>
                    <button
                      onclick={() => handleRemoveDirectory(dir.local_path)}
                      class="p-2 rounded-lg text-slate-400 dark:text-[#8e918f] hover:text-rose-600 hover:bg-rose-50 dark:hover:bg-rose-950/40 transition-colors"
                      title="Stop syncing folder"
                    >
                      <Trash class="w-4 h-4" />
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        </div>
      {/if}

      <!-- ==================== TAB 2: GOOGLE DRIVE (CLOUD AUTH) ==================== -->
      {#if activeTab === 2}
        <div class="flex flex-col gap-6 max-w-4xl mx-auto">
          <div class="flex flex-col pb-1 border-b border-slate-100 dark:border-[#2d2f31]">
            <h1 class="text-2xl font-bold tracking-tight text-slate-800 dark:text-[#e3e3e3]">Google Drive Account</h1>
            <p class="text-xs text-slate-500 dark:text-[#8e918f] mt-1">Cloud authorization and storage quota breakdown.</p>
          </div>

          {#if !state.is_connected}
            <div class="p-8 rounded-2xl border border-blue-200 dark:border-[#004a77] bg-blue-50/40 dark:bg-[#004a77]/20 flex flex-col items-center justify-center text-center gap-4">
              <div class="p-4 rounded-full bg-white dark:bg-[#282a2c] text-[#1967d2] dark:text-[#7cacf8] shadow-xs">
                <Cloud class="w-8 h-8" />
              </div>
              <div class="flex flex-col gap-1 max-w-md">
                <h3 class="text-base font-bold text-slate-900 dark:text-[#e3e3e3]">Sign in to Google Drive</h3>
                <p class="text-xs text-slate-600 dark:text-[#c4c7c5]">
                  Authenticate your Google account using your configured OAuth application in <code class="px-1 py-0.5 rounded bg-slate-200 dark:bg-[#282a2c]">~/.config/xsync/config.toml</code>.
                </p>
              </div>
              <button
                onclick={handleLogin}
                class="flex items-center gap-2 px-6 py-2.5 rounded-full bg-[#1967d2] hover:bg-[#1557bf] dark:bg-[#0b57d0] dark:hover:bg-[#1b6ef3] text-white font-semibold text-xs transition-colors shadow-xs"
              >
                <User class="w-4 h-4" />
                <span>Sign In via Browser</span>
              </button>
            </div>
          {:else}
            <!-- Profile Card -->
            <div class="flex items-center justify-between p-6 rounded-2xl border border-slate-200/80 dark:border-[#444746]/40 bg-white dark:bg-[#282a2c] shadow-xs">
              <div class="flex items-center gap-4">
                <div class="w-14 h-14 rounded-2xl flex items-center justify-center bg-[#1967d2] dark:bg-[#0b57d0] text-white font-bold text-xl shadow-xs">
                  {state.user_name ? state.user_name.charAt(0).toUpperCase() : 'G'}
                </div>
                <div class="flex flex-col">
                  <div class="flex items-center gap-2">
                    <span class="font-bold text-base text-slate-900 dark:text-[#e3e3e3]">{state.user_name}</span>
                    <span class="flex items-center gap-1 text-[11px] font-semibold px-2 py-0.5 rounded-full bg-emerald-100 dark:bg-[#1a2e22] text-emerald-700 dark:text-[#6dd58c] border border-emerald-200 dark:border-[#2b4c37]">
                      <Check class="w-3 h-3" /> Connected
                    </span>
                  </div>
                  <span class="text-xs text-slate-400 dark:text-[#8e918f]">{state.user_email}</span>
                </div>
              </div>

              <button
                onclick={handleLogout}
                class="flex items-center gap-2 px-4 py-2 rounded-xl border border-slate-200 dark:border-[#444746]/50 text-xs font-semibold text-slate-700 dark:text-[#c4c7c5] hover:bg-rose-50 hover:text-rose-600 dark:hover:bg-rose-950/30 transition-colors"
              >
                <span>Disconnect</span>
              </button>
            </div>

            <!-- Detailed Quota Card -->
            <div class="flex flex-col gap-4 p-6 rounded-2xl border border-slate-200/80 dark:border-[#444746]/40 bg-white dark:bg-[#282a2c] shadow-xs">
              <div class="flex items-center justify-between pb-2 border-b border-slate-100 dark:border-[#2d2f31]">
                <h3 class="text-sm font-bold text-slate-900 dark:text-[#e3e3e3]">Cloud Storage Quota</h3>
                <span class="text-xs font-mono font-semibold text-[#1967d2] dark:text-[#7cacf8]">
                  {Math.round(state.storage_ratio * 100)}% Used
                </span>
              </div>

              <div class="flex flex-col gap-2">
                <div class="h-2.5 w-full rounded-full bg-slate-100 dark:bg-[#1e1f20] overflow-hidden">
                  <div
                    class="h-full rounded-full transition-all duration-300 bg-[#1967d2] dark:bg-[#7cacf8]"
                    style="width: {Math.max(state.storage_ratio * 100, 2)}%"
                  ></div>
                </div>
                <div class="flex items-center justify-between text-xs text-slate-500 dark:text-[#8e918f]">
                  <span>{state.storage_used_str} Used</span>
                  <span>{state.storage_total_str} Total Capacity</span>
                </div>
              </div>
            </div>
          {/if}
        </div>
      {/if}

      <!-- ==================== TAB 3: DIFF & MERGE ==================== -->
      {#if activeTab === 3}
        <div class="flex flex-col gap-6 max-w-4xl mx-auto">
          <div class="flex flex-col pb-1 border-b border-slate-100 dark:border-[#2d2f31]">
            <h1 class="text-2xl font-bold tracking-tight text-slate-800 dark:text-[#e3e3e3]">Drive Diff & Merge</h1>
            <p class="text-xs text-slate-500 dark:text-[#8e918f] mt-1">Compare checksums and merge missing items between two Google Drive folders.</p>
          </div>

          <div class="flex flex-col gap-4 p-5 rounded-2xl border border-slate-200/80 dark:border-[#444746]/40 bg-white dark:bg-[#282a2c] shadow-xs">
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div class="flex flex-col gap-1.5">
                <label for="diff-src" class="text-xs font-medium text-slate-600 dark:text-[#c4c7c5]">Source Folder Name or ID</label>
                <input
                  id="diff-src"
                  type="text"
                  bind:value={diffFolder1}
                  placeholder="root or folder name"
                  class="px-3 py-2 text-xs font-mono rounded-xl border border-slate-200 dark:border-[#444746]/50 bg-slate-50 dark:bg-[#1e1f20] text-slate-900 dark:text-[#e3e3e3] focus:outline-none focus:ring-2 focus:ring-[#1967d2]"
                />
              </div>

              <div class="flex flex-col gap-1.5">
                <label for="diff-dest" class="text-xs font-medium text-slate-600 dark:text-[#c4c7c5]">Target Folder Name or ID</label>
                <input
                  id="diff-dest"
                  type="text"
                  bind:value={diffFolder2}
                  placeholder="Backup_Folder"
                  class="px-3 py-2 text-xs font-mono rounded-xl border border-slate-200 dark:border-[#444746]/50 bg-slate-50 dark:bg-[#1e1f20] text-slate-900 dark:text-[#e3e3e3] focus:outline-none focus:ring-2 focus:ring-[#1967d2]"
                />
              </div>
            </div>

            <div class="flex items-center gap-3 pt-2">
              <button
                onclick={handleRunDiff}
                disabled={isDiffing || !diffFolder2}
                class="flex items-center gap-2 px-4 py-2 rounded-full bg-[#1967d2] hover:bg-[#1557bf] dark:bg-[#0b57d0] dark:hover:bg-[#1b6ef3] text-white text-xs font-semibold disabled:opacity-50 transition-colors shadow-xs"
              >
                {#if isDiffing}
                  <RefreshCw class="w-3.5 h-3.5 animate-spin" />
                {:else}
                  <GitCompare class="w-3.5 h-3.5" />
                {/if}
                <span>Compare Folders</span>
              </button>

              <button
                onclick={handleRunMerge}
                disabled={isMerging || !diffFolder2 || diffResults.length === 0}
                class="flex items-center gap-2 px-4 py-2 rounded-full border border-slate-300 dark:border-[#444746]/60 hover:bg-slate-100 dark:hover:bg-[#333538] text-xs font-semibold text-slate-700 dark:text-[#e3e3e3] disabled:opacity-50 transition-colors"
              >
                {#if isMerging}
                  <RefreshCw class="w-3.5 h-3.5 animate-spin" />
                {/if}
                <span>Merge Missing to Target</span>
              </button>
            </div>
          </div>

          {#if diffMessage}
            <div class="p-4 rounded-xl bg-blue-50 dark:bg-[#004a77]/30 border border-blue-200 dark:border-[#005c91] text-xs text-[#1967d2] dark:text-[#c2e7ff] font-medium">
              {diffMessage}
            </div>
          {/if}

          {#if diffResults.length > 0}
            <div class="flex flex-col rounded-2xl border border-slate-200/80 dark:border-[#444746]/40 bg-white dark:bg-[#282a2c] shadow-xs overflow-hidden">
              <div class="px-5 py-3 border-b border-slate-100 dark:border-[#2d2f31] flex items-center justify-between">
                <span class="text-xs font-semibold text-slate-700 dark:text-[#e3e3e3]">Differences Detected</span>
                <span class="text-xs text-slate-500 dark:text-[#8e918f] font-mono">{diffResults.length} files differing</span>
              </div>
              <div class="divide-y divide-slate-100 dark:divide-[#2d2f31] max-h-96 overflow-y-auto">
                {#each diffResults as diff}
                  <div class="p-3 flex items-center justify-between text-xs hover:bg-[#e8f0fe]/50 dark:hover:bg-[#333538]">
                    <span class="font-mono text-slate-800 dark:text-[#e3e3e3] truncate">{diff.path}</span>
                    <span class="px-2 py-0.5 rounded text-[10px] font-bold uppercase
                      {diff.side === 'only_in_source'
                        ? 'bg-blue-100 text-[#1967d2] dark:bg-[#004a77] dark:text-[#c2e7ff]'
                        : diff.side === 'only_in_dest'
                        ? 'bg-amber-100 text-amber-700 dark:bg-[#392c1d] dark:text-[#ffddb3]'
                        : 'bg-rose-100 text-rose-700 dark:bg-[#3b1d24] dark:text-[#f87171]'}"
                    >
                      {diff.side.replace(/_/g, ' ')}
                    </span>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {/if}

      <!-- ==================== TAB 4: RECENT ACTIVITY ==================== -->
      {#if activeTab === 4}
        <div class="flex flex-col gap-6 max-w-4xl mx-auto">
          <div class="flex items-center justify-between pb-1 border-b border-slate-100 dark:border-[#2d2f31]">
            <div class="flex flex-col">
              <h1 class="text-2xl font-bold tracking-tight text-slate-800 dark:text-[#e3e3e3]">Recent Activity</h1>
              <p class="text-xs text-slate-500 dark:text-[#8e918f] mt-1">Chronicle of files synced with Google Drive.</p>
            </div>
            <div class="flex items-center gap-2">
              <button
                onclick={handleSyncAll}
                disabled={state.is_syncing}
                class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-full bg-[#1967d2] hover:bg-[#1557bf] dark:bg-[#0b57d0] dark:hover:bg-[#1b6ef3] text-white text-xs font-semibold shadow-xs transition-colors cursor-pointer disabled:opacity-50"
                title="Trigger synchronization pass now"
              >
                <RefreshCw class="w-3.5 h-3.5 {state.is_syncing ? 'animate-spin' : ''}" />
                <span>{state.is_syncing ? 'Syncing...' : 'Sync Now'}</span>
              </button>
              <button
                onclick={refreshAll}
                class="flex items-center gap-1.5 px-3 py-1.5 rounded-full border border-slate-200 dark:border-[#444746]/50 text-xs font-semibold text-slate-700 dark:text-[#c4c7c5] hover:bg-slate-100 dark:hover:bg-[#282a2c] transition-colors cursor-pointer"
              >
                <RefreshCw class="w-3.5 h-3.5" />
                <span>Refresh</span>
              </button>
            </div>
          </div>

          <div class="flex flex-col rounded-2xl border border-slate-200/80 dark:border-[#444746]/40 bg-white dark:bg-[#282a2c] shadow-xs overflow-hidden">
            <div class="divide-y divide-slate-100 dark:divide-[#2d2f31] max-h-[500px] overflow-y-auto">
              {#if recentActivity.length === 0}
                <div class="py-16 flex flex-col items-center justify-center text-slate-400 dark:text-[#8e918f] gap-2">
                  <Check class="w-8 h-8 text-emerald-500" />
                  <span class="text-sm font-semibold">All files up to date</span>
                  <span class="text-xs text-slate-400">Newly synced files will appear here automatically</span>
                </div>
              {:else}
                {#each recentActivity as item}
                  <div class="p-3.5 flex items-center justify-between hover:bg-[#e8f0fe]/60 dark:hover:bg-[#333538] transition-colors">
                    <div class="flex items-center gap-3 min-w-0">
                      <div class="p-2 rounded-lg bg-emerald-50 dark:bg-[#1a2e22] text-emerald-600 dark:text-[#6dd58c]">
                        <Check class="w-4 h-4" />
                      </div>
                      <div class="flex flex-col min-w-0">
                        <span class="text-xs font-semibold font-mono text-slate-900 dark:text-[#e3e3e3] truncate">
                          {item.rel_path}
                        </span>
                        <span class="text-[11px] text-slate-400 dark:text-[#8e918f]">
                          {item.size_str} • Synced
                        </span>
                      </div>
                    </div>
                    <div class="flex items-center gap-1.5 text-xs text-slate-400 dark:text-[#8e918f] flex-shrink-0">
                      <Clock class="w-3.5 h-3.5 text-emerald-600 dark:text-[#6dd58c]" />
                      <span class="font-medium text-slate-600 dark:text-[#c4c7c5]">{item.time_str}</span>
                    </div>
                  </div>
                {/each}
              {/if}
            </div>
          </div>
        </div>
      {/if}

      <!-- ==================== TAB 5: SETTINGS ==================== -->
      {#if activeTab === 5}
        <div class="flex flex-col gap-6 max-w-4xl mx-auto">
          <div class="flex flex-col pb-1 border-b border-slate-100 dark:border-[#2d2f31]">
            <h1 class="text-2xl font-bold tracking-tight text-slate-800 dark:text-[#e3e3e3]">Settings & Preferences</h1>
            <p class="text-xs text-slate-500 dark:text-[#8e918f] mt-1">Configure desktop appearance and background sync system daemon.</p>
          </div>

          <!-- Appearance Card -->
          <div class="flex items-center justify-between p-5 rounded-2xl border border-slate-200/80 dark:border-[#444746]/40 bg-white dark:bg-[#282a2c] shadow-xs">
            <div class="flex flex-col">
              <span class="text-sm font-bold text-slate-900 dark:text-[#e3e3e3]">Interface Theme</span>
              <span class="text-xs text-slate-400 dark:text-[#8e918f]">Select Google Drive light or dark desktop mode</span>
            </div>
            <div class="flex items-center gap-2">
              <button
                onclick={() => { if (isDarkMode) toggleTheme(); }}
                class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-full text-xs font-semibold border transition-colors
                  {!isDarkMode
                    ? 'bg-[#1967d2] text-white border-[#1967d2] shadow-xs'
                    : 'border-slate-300 dark:border-[#444746]/60 text-slate-600 dark:text-[#c4c7c5]'}"
              >
                <Sun class="w-3.5 h-3.5" />
                <span>Light Mode</span>
              </button>
              <button
                onclick={() => { if (!isDarkMode) toggleTheme(); }}
                class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-full text-xs font-semibold border transition-colors
                  {isDarkMode
                    ? 'bg-[#004a77] text-[#c2e7ff] border-transparent font-medium shadow-xs'
                    : 'border-slate-300 dark:border-[#444746]/60 text-slate-600 dark:text-[#c4c7c5]'}"
              >
                <Moon class="w-3.5 h-3.5" />
                <span>Dark Mode</span>
              </button>
            </div>
          </div>

          <!-- Systemd Background Service Card -->
          <div class="flex flex-col gap-4 p-5 rounded-2xl border border-slate-200/80 dark:border-[#444746]/40 bg-white dark:bg-[#282a2c] shadow-xs">
            <div class="flex items-center justify-between pb-2 border-b border-slate-100 dark:border-[#2d2f31]">
              <div class="flex items-center gap-2">
                <ShieldCheck class="w-4 h-4 text-[#1967d2] dark:text-[#7cacf8]" />
                <h3 class="text-sm font-bold text-slate-900 dark:text-[#e3e3e3]">Background Sync Daemon (systemd)</h3>
              </div>
              <span class="text-xs text-slate-400 dark:text-[#8e918f] font-mono">xsync.service</span>
            </div>

            <p class="text-xs text-slate-500 dark:text-[#8e918f]">
              Run xsync continuously in the background using Linux user-level systemd service without needing the GUI to remain open.
            </p>

            <div class="flex flex-wrap gap-2.5 pt-1">
              <button
                onclick={() => handleSystemd('install')}
                class="flex items-center gap-1.5 px-4 py-2 rounded-full text-xs font-semibold bg-[#1967d2] hover:bg-[#1557bf] dark:bg-[#0b57d0] dark:hover:bg-[#1b6ef3] text-white transition-colors shadow-xs"
              >
                <ShieldCheck class="w-3.5 h-3.5" />
                <span>Install & Enable at Boot</span>
              </button>

              <button
                onclick={() => handleSystemd('start')}
                class="flex items-center gap-1.5 px-4 py-2 rounded-full text-xs font-semibold border border-slate-200 dark:border-[#444746]/60 hover:bg-slate-100 dark:hover:bg-[#333538] text-slate-700 dark:text-[#e3e3e3] transition-colors"
              >
                <Play class="w-3.5 h-3.5 text-emerald-500" />
                <span>Start Daemon</span>
              </button>

              <button
                onclick={() => handleSystemd('stop')}
                class="flex items-center gap-1.5 px-4 py-2 rounded-full text-xs font-semibold border border-slate-200 dark:border-[#444746]/60 hover:bg-slate-100 dark:hover:bg-[#333538] text-slate-700 dark:text-[#e3e3e3] transition-colors"
              >
                <Pause class="w-3.5 h-3.5 text-amber-500" />
                <span>Stop Daemon</span>
              </button>

              <button
                onclick={() => handleSystemd('restart')}
                class="flex items-center gap-1.5 px-4 py-2 rounded-full text-xs font-semibold border border-slate-200 dark:border-[#444746]/60 hover:bg-slate-100 dark:hover:bg-[#333538] text-slate-700 dark:text-[#e3e3e3] transition-colors"
              >
                <RefreshCw class="w-3.5 h-3.5 text-blue-500" />
                <span>Restart Daemon</span>
              </button>
            </div>
          </div>
        </div>
      {/if}
    </div>
  </main>
</div>

<!-- ======================== ADD FOLDER MODAL ======================== -->
{#if showAddFolderModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
    <div class="w-full max-w-md rounded-3xl bg-white dark:bg-[#1e1f20] border border-slate-200 dark:border-[#444746]/60 shadow-2xl p-6 flex flex-col gap-5">
      <div class="flex items-center justify-between pb-2 border-b border-slate-100 dark:border-[#2d2f31]">
        <div class="flex items-center gap-2.5">
          <div class="p-2 rounded-xl bg-blue-50 dark:bg-[#282a2c] text-[#1967d2] dark:text-[#7cacf8]">
            <FolderPlus class="w-5 h-5" />
          </div>
          <div class="flex flex-col">
            <h3 class="text-sm font-bold text-slate-900 dark:text-[#e3e3e3]">Add Sync Folder</h3>
            <span class="text-xs text-slate-400 dark:text-[#8e918f]">Select a local directory to synchronize</span>
          </div>
        </div>
        <button onclick={() => showAddFolderModal = false} class="p-1.5 rounded-full text-slate-400 dark:text-[#8e918f] hover:bg-slate-100 dark:hover:bg-[#282a2c]">
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="flex flex-col gap-4">
        <div class="flex flex-col gap-1.5">
          <label for="modal-folder-path" class="text-xs font-medium text-slate-700 dark:text-[#c4c7c5]">Local Directory</label>
          <div class="flex gap-2">
            <input
              id="modal-folder-path"
              type="text"
              bind:value={newLocalPath}
              placeholder="/home/user/MyFolder"
              class="flex-1 px-3.5 py-2 text-xs font-mono rounded-xl border border-slate-200 dark:border-[#444746]/50 bg-slate-50 dark:bg-[#282a2c] text-slate-900 dark:text-[#e3e3e3] focus:outline-none focus:ring-2 focus:ring-[#1967d2]"
            />
            <button
              onclick={handleBrowseFolder}
              class="px-4 py-2 text-xs font-semibold rounded-xl border border-slate-200 dark:border-[#444746]/50 hover:bg-slate-100 dark:hover:bg-[#282a2c] text-slate-700 dark:text-[#e3e3e3] transition-colors"
            >
              Browse...
            </button>
          </div>
        </div>

        <div class="flex flex-col gap-1.5">
          <label for="modal-folder-name" class="text-xs font-medium text-slate-700 dark:text-[#c4c7c5]">Google Drive Folder Name</label>
          <input
            id="modal-folder-name"
            type="text"
            bind:value={newRemoteName}
            placeholder="Folder name on Google Drive"
            class="px-3.5 py-2 text-xs rounded-xl border border-slate-200 dark:border-[#444746]/50 bg-slate-50 dark:bg-[#282a2c] text-slate-900 dark:text-[#e3e3e3] focus:outline-none focus:ring-2 focus:ring-[#1967d2]"
          />
        </div>
      </div>

      <div class="flex items-center justify-end gap-2.5 pt-3 border-t border-slate-100 dark:border-[#2d2f31]">
        <button
          onclick={() => showAddFolderModal = false}
          class="px-4 py-2 rounded-full text-xs font-semibold text-slate-600 dark:text-[#c4c7c5] hover:bg-slate-100 dark:hover:bg-[#282a2c] transition-colors"
        >
          Cancel
        </button>
        <button
          onclick={handleAddDirectory}
          disabled={isAddingFolder || !newLocalPath}
          class="flex items-center gap-1.5 px-5 py-2 rounded-full text-xs font-bold bg-[#1967d2] hover:bg-[#1557bf] dark:bg-[#0b57d0] dark:hover:bg-[#1b6ef3] text-white disabled:opacity-50 transition-colors shadow-xs"
        >
          {#if isAddingFolder}
            <RefreshCw class="w-3.5 h-3.5 animate-spin" />
          {:else}
            <Plus class="w-3.5 h-3.5" />
          {/if}
          <span>Start Syncing Folder</span>
        </button>
      </div>
    </div>
  </div>
{/if}
