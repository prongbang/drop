<script lang="ts">
	type Peer = { id: string; name: string };
	type Transfer = {
		id: number;
		peer: string;
		name: string;
		size: number;
		stage: 'pending' | 'accepted' | 'declined' | 'ready';
		incoming: boolean;
	};
	type Item = { name: string; size: number; dir: boolean };
	type IconKind = 'folder' | 'image' | 'media' | 'pdf' | 'code' | 'sheet' | 'text' | 'archive' | 'file';

	let me = $state('');
	let myName = $state('');
	let peers = $state<Peer[]>([]);
	let transfers = $state<Transfer[]>([]);
	let items = $state<Item[]>([]);
	let path = $state('');
	let search = $state('');
	let dragging = $state('');
	let dragActive = $state(false);
	let isDark = $state(false);
	let error = $state('');
	let listError = $state('');
	let picker: HTMLInputElement;
	let uploadPicker: HTMLInputElement;
	let target = '';

	// files wait here until the other side accepts
	const staged = new Map<number, File>();

	const ask = $derived(transfers.find((t) => t.incoming && t.stage === 'pending'));

	function apply(state: { name: string; peers: Peer[]; transfers: Transfer[] }) {
		myName = state.name;
		peers = state.peers;
		transfers = state.transfers;
		for (const transfer of transfers) {
			if (!transfer.incoming && transfer.stage === 'accepted' && staged.has(transfer.id)) {
				upload(transfer.id);
			}
		}
	}

	function createPeerId() {
		try {
			const id = globalThis.crypto?.randomUUID?.();
			if (id) return id;
		} catch {
			// randomUUID may be unavailable outside secure contexts, such as HTTP on a LAN.
		}
		return `peer-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
	}

	async function listDir() {
		const at = path;
		try {
			const response = await fetch(`/api/dir?path=${encodeURIComponent(at)}`);
			if (!response.ok) throw new Error(`Directory API returned HTTP ${response.status}`);
			const listing = await response.json();
			// a poll that started before the last click must not drag the view back
			if (at === path) {
				items = listing.entries;
				listError = '';
			}
		} catch (e) {
			if (at === path) {
				listError = e instanceof Error ? `Could not load files: ${e.message}` : 'Could not reach the directory API';
			}
		}
	}

	$effect(() => {
		me = localStorage.getItem('peer') ?? createPeerId();
		localStorage.setItem('peer', me);

		// presence and transfers are pushed; the stream staying open is what keeps us listed
		const events = new EventSource(`/api/events?me=${me}`);
		events.onmessage = (message) => apply(JSON.parse(message.data));

		// ponytail: the directory is still polled, nothing tells the server when a file lands in it
		listDir();
		const timer = setInterval(listDir, 2000);
		return () => {
			events.close();
			clearInterval(timer);
		};
	});

	async function offer(to: string, list: FileList | null) {
		error = '';
		for (const file of Array.from(list ?? [])) {
			const res = await fetch(`/api/offer?me=${me}`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({
					to,
					name: file.name,
					size: file.size,
					mime: file.type || 'application/octet-stream'
				})
			});
			if (res.ok) staged.set(await res.json(), file);
			else error = `${file.name} could not be offered (over 100 MB, or the device left)`;
		}
	}

	async function upload(id: number) {
		const file = staged.get(id);
		if (!file) return;
		staged.delete(id); // drop it first so the next poll cannot send it twice
		await fetch(`/api/upload/${id}?me=${me}`, {
			method: 'POST',
			headers: { 'content-type': file.type || 'application/octet-stream' },
			body: file
		});
	}

	async function respond(id: number, accept: boolean) {
		await fetch(`/api/respond?me=${me}`, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({ id, accept })
		});
	}

	async function uploadFiles(list: FileList | null) {
		const files = Array.from(list ?? []);
		if (!files.length) return;
		error = '';
		try {
			for (const file of files) {
				const res = await fetch(
					`/api/files?path=${encodeURIComponent(path)}&name=${encodeURIComponent(file.name)}`,
					{ method: 'POST', body: file }
				);
				if (!res.ok) throw new Error(`${file.name} could not be uploaded`);
			}
			await listDir();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Files could not be uploaded';
		}
	}

	function status(transfer: Transfer) {
		if (transfer.stage === 'declined') return 'declined';
		if (transfer.stage === 'ready') return transfer.incoming ? '' : 'sent';
		if (transfer.stage === 'accepted') return 'sending…';
		return transfer.incoming ? 'waiting for you' : 'waiting for accept';
	}

	const segments = $derived(path.split('/').filter(Boolean));
	const visibleItems = $derived(
		items.filter((item) => item.name.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()))
	);
	$effect(() => {
		const savedTheme = localStorage.getItem('theme');
		const dark = savedTheme ? savedTheme === 'dark' : window.matchMedia('(prefers-color-scheme: dark)').matches;
		isDark = dark;
		document.documentElement.classList.toggle('dark', dark);
	});

	function go(next: string) {
		path = next;
		listDir();
	}

	function toggleTheme() {
		isDark = !isDark;
		localStorage.setItem('theme', isDark ? 'dark' : 'light');
		document.documentElement.classList.toggle('dark', isDark);
	}

	function href(name: string) {
		return '/' + [...segments, name].map(encodeURIComponent).join('/');
	}

	function iconKind(item: Item): IconKind {
		if (item.dir) return 'folder';
		const extension = item.name.split('.').pop()?.toLowerCase() ?? '';
		if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'avif', 'bmp'].includes(extension)) return 'image';
		if (['mp4', 'mov', 'mkv', 'webm', 'mp3', 'wav', 'flac', 'aac', 'm4a'].includes(extension)) return 'media';
		if (extension === 'pdf') return 'pdf';
		if (['js', 'ts', 'jsx', 'tsx', 'html', 'css', 'json', 'rs', 'go', 'py', 'sh', 'yml', 'yaml', 'toml'].includes(extension)) return 'code';
		if (['csv', 'xls', 'xlsx', 'ods'].includes(extension)) return 'sheet';
		if (['txt', 'md', 'doc', 'docx', 'rtf'].includes(extension)) return 'text';
		if (['zip', 'tar', 'gz', '7z', 'rar'].includes(extension)) return 'archive';
		return 'file';
	}

	function iconColor(item: Item) {
		const colors: Record<IconKind, string> = {
			folder: 'bg-amber-100 text-amber-600 dark:bg-amber-400/15 dark:text-amber-300',
			image: 'bg-fuchsia-100 text-fuchsia-600 dark:bg-fuchsia-400/15 dark:text-fuchsia-300',
			media: 'bg-violet-100 text-violet-600 dark:bg-violet-400/15 dark:text-violet-300',
			pdf: 'bg-rose-100 text-rose-600 dark:bg-rose-400/15 dark:text-rose-300',
			code: 'bg-sky-100 text-sky-600 dark:bg-sky-400/15 dark:text-sky-300',
			sheet: 'bg-emerald-100 text-emerald-600 dark:bg-emerald-400/15 dark:text-emerald-300',
			text: 'bg-blue-100 text-blue-600 dark:bg-blue-400/15 dark:text-blue-300',
			archive: 'bg-orange-100 text-orange-600 dark:bg-orange-400/15 dark:text-orange-300',
			file: 'bg-slate-100 text-slate-500 dark:bg-slate-400/15 dark:text-slate-300'
		};
		return colors[iconKind(item)];
	}

	function size(bytes: number) {
		const units = ['B', 'KB', 'MB', 'GB'];
		let n = bytes;
		let unit = 0;
		while (n >= 1024 && unit < units.length - 1) {
			n /= 1024;
			unit++;
		}
		return `${n < 10 && unit > 0 ? n.toFixed(1) : Math.round(n)} ${units[unit]}`;
	}
</script>

<main class="mx-auto grid min-h-dvh w-full max-w-7xl grid-cols-1 gap-6 bg-slate-50 p-4 text-slate-900 sm:p-6 md:grid-cols-[240px_minmax(0,1fr)] md:gap-8 md:p-8 dark:bg-slate-950 dark:text-slate-100">
	<input
		bind:this={picker}
		type="file"
		multiple
		class="hidden"
		onchange={(e) => {
			offer(target, e.currentTarget.files);
			e.currentTarget.value = '';
		}}
	/>
	<input
		bind:this={uploadPicker}
		type="file"
		multiple
		class="hidden"
		onchange={(e) => {
			uploadFiles(e.currentTarget.files);
			e.currentTarget.value = '';
		}}
	/>

	<aside class="order-2 flex flex-col gap-6 md:order-1 md:py-2">
		<div class="flex items-center justify-between gap-3">
			<div class="flex items-center gap-3">
				<span class="flex h-11 w-11 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 to-violet-500 text-white shadow-lg shadow-indigo-500/20">
					<svg aria-hidden="true" viewBox="0 0 24 24" class="h-6 w-6" fill="currentColor"><path d="M12 2.75c-1.6 2.64-6.5 7.87-6.5 12.1a6.5 6.5 0 1 0 13 0c0-4.23-4.9-9.46-6.5-12.1Zm0 17a4.2 4.2 0 0 1-4.2-4.2c0-1.76 1.36-4.25 3.1-6.73-.05 2.13.44 3.34 1.51 4.29.69.62 1.19 1.31 1.19 2.4A1.6 1.6 0 0 1 12 17.1a1.6 1.6 0 0 1-1.6-1.6.9.9 0 0 0-1.8 0 3.4 3.4 0 0 0 6.8 0c0-1.67-.77-2.86-1.78-3.77-.43-.39-.71-.83-.86-1.48 1.95 2.56 3.44 5.1 3.44 7.3A4.2 4.2 0 0 1 12 19.75Z" /></svg>
				</span>
				<div>
					<p class="text-lg font-semibold tracking-tight">Drop</p>
					<p class="text-xs text-slate-500 dark:text-slate-400">Local file sharing</p>
				</div>
			</div>
			<button
				class="inline-flex h-11 w-11 items-center justify-center rounded-xl border border-slate-200 bg-white text-slate-600 transition hover:bg-slate-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-500 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-300 dark:hover:bg-slate-800"
				aria-label={isDark ? 'Switch to light theme' : 'Switch to dark theme'}
				title={isDark ? 'Switch to light theme' : 'Switch to dark theme'}
				onclick={toggleTheme}
			>
				{#if isDark}
					<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="4" /><path d="M12 2v2m0 16v2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42M2 12h2m16 0h2M4.93 19.07l1.42-1.42m11.3-11.3 1.42-1.42" /></svg>
				{:else}
					<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20.2 15.1A8.5 8.5 0 0 1 8.9 3.8 8.6 8.6 0 1 0 20.2 15.1Z" /></svg>
				{/if}
			</button>
		</div>

		<section class="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm shadow-slate-900/[0.03] dark:border-slate-800 dark:bg-slate-900">
			<div class="mb-4 flex items-center justify-between gap-3">
				<h2 class="text-sm font-semibold">Devices nearby</h2>
				<span class="rounded-full bg-emerald-50 px-2.5 py-1 text-xs font-medium text-emerald-700 dark:bg-emerald-400/10 dark:text-emerald-300">{peers.length + (myName ? 1 : 0)} online</span>
			</div>
			<div class="mb-2 flex items-center gap-3 rounded-xl bg-slate-50 px-3 py-3 dark:bg-slate-800/70">
				<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-indigo-100 text-indigo-600 dark:bg-indigo-400/15 dark:text-indigo-300">
					<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="13" rx="1.5" /><path d="M8 21h8m-4-4v4" /></svg>
				</span>
				<span class="min-w-0 flex-1">
					<span class="block truncate text-sm font-medium">{myName || 'Connecting…'}</span>
					<span class="flex items-center gap-1.5 text-xs text-slate-500 dark:text-slate-400"><span class="h-1.5 w-1.5 rounded-full bg-emerald-500"></span>This device</span>
				</span>
			</div>
			{#each peers as peer (peer.id)}
				<button
					class="mb-1 flex min-h-14 w-full items-center gap-3 rounded-xl px-3 py-2 text-left transition {dragging === peer.id ? 'bg-indigo-50 ring-2 ring-indigo-400 dark:bg-indigo-400/10' : 'hover:bg-slate-50 dark:hover:bg-slate-800'} focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-500"
					onclick={() => { target = peer.id; picker.click(); }}
					ondragover={(e) => { e.preventDefault(); dragging = peer.id; }}
					ondragleave={() => (dragging = '')}
					ondrop={(e) => { e.preventDefault(); dragging = ''; offer(peer.id, e.dataTransfer?.files ?? null); }}
				>
					<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-sky-100 text-sky-600 dark:bg-sky-400/15 dark:text-sky-300">
						<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="13" rx="1.5" /><path d="M8 21h8m-4-4v4" /></svg>
					</span>
					<span class="min-w-0 flex-1 truncate text-sm">{peer.name}</span>
					<span class="shrink-0 text-xs font-medium text-indigo-600 dark:text-indigo-300">Send</span>
				</button>
			{:else}
				<p class="rounded-xl border border-dashed border-slate-200 px-3 py-4 text-xs leading-5 text-slate-500 dark:border-slate-700 dark:text-slate-400">Open Drop on another device using the same Drop address shown on the Mac.</p>
			{/each}
		</section>

		{#if transfers.length}
			<section class="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm shadow-slate-900/[0.03] dark:border-slate-800 dark:bg-slate-900">
				<h2 class="mb-3 text-sm font-semibold">Recent transfers</h2>
				{#each transfers as transfer (transfer.id)}
					<div class="flex items-center gap-2 border-t border-slate-100 py-3 first:border-0 dark:border-slate-800">
						<span class="min-w-0 flex-1">
							<span class="block truncate text-sm font-medium">{transfer.name}</span>
							<span class="block truncate text-xs text-slate-500 dark:text-slate-400">{transfer.incoming ? 'From' : 'To'} {transfer.peer} · {size(transfer.size)}</span>
						</span>
						{#if transfer.incoming && transfer.stage === 'ready'}
							<a class="rounded-lg px-2.5 py-2 text-xs font-semibold text-indigo-600 hover:bg-indigo-50 dark:text-indigo-300 dark:hover:bg-indigo-400/10" href="/api/transfer/{transfer.id}?me={me}" download={transfer.name}>Download</a>
						{:else}
						<span class="text-xs text-slate-500 dark:text-slate-400">{status(transfer)}</span>
						{/if}
					</div>
				{/each}
			</section>
		{/if}
	</aside>

	<section class="order-1 flex min-w-0 flex-col gap-6 md:order-2">
		<header class="flex flex-wrap items-center justify-between gap-4">
			<div>
				<p class="mb-1 text-sm font-medium text-indigo-600 dark:text-indigo-300">FILE MANAGER</p>
				<h1 class="text-3xl font-semibold tracking-tight sm:text-4xl">Your files</h1>
			</div>
		</header>

		{#if error}
			<p role="alert" class="rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-800 dark:border-rose-900 dark:bg-rose-950/60 dark:text-rose-200">{error}</p>
		{/if}
		{#if listError}
			<p role="alert" class="rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-800 dark:border-rose-900 dark:bg-rose-950/60 dark:text-rose-200">{listError}</p>
		{/if}

		<button
			type="button"
			class="flex min-h-52 w-full flex-col items-center justify-center rounded-3xl border-2 border-dashed px-6 py-8 text-center transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-500 {dragActive ? 'border-indigo-500 bg-indigo-50 dark:bg-indigo-400/10' : 'border-indigo-200 bg-indigo-50/60 hover:border-indigo-400 hover:bg-indigo-50 dark:border-indigo-900 dark:bg-indigo-950/30 dark:hover:bg-indigo-950/60'}"
			ondragenter={(e) => { e.preventDefault(); dragActive = true; }}
			ondragover={(e) => e.preventDefault()}
			ondragleave={(e) => { if (e.currentTarget === e.target) dragActive = false; }}
			ondrop={(e) => { e.preventDefault(); dragActive = false; uploadFiles(e.dataTransfer?.files ?? null); }}
			onclick={() => uploadPicker.click()}
		>
			<span class="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-indigo-100 text-indigo-600 dark:bg-indigo-400/15 dark:text-indigo-300">
				<svg aria-hidden="true" viewBox="0 0 24 24" class="h-7 w-7" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M7 18a4 4 0 0 1-.6-7.95A5.5 5.5 0 0 1 17 8.5h.5A3.5 3.5 0 0 1 18 15.49M12 21V12m0 0-3.5 3.5M12 12l3.5 3.5" /></svg>
			</span>
			<span class="text-lg font-semibold">{dragActive ? 'Drop files to upload' : 'Drop files here to upload'}</span>
			<span class="mt-1 text-sm text-slate-500 dark:text-slate-400">or choose files from your device</span>
			<span class="mt-4 inline-flex min-h-11 items-center rounded-lg border border-indigo-200 bg-white px-4 text-sm font-semibold text-indigo-700 shadow-sm dark:border-indigo-800 dark:bg-slate-900 dark:text-indigo-300">Browse files</span>
		</button>

		<section class="flex min-w-0 flex-col gap-3">
			<label for="file-search" class="sr-only">Search files and folders</label>
			<div class="relative">
				<svg aria-hidden="true" viewBox="0 0 24 24" class="pointer-events-none absolute left-4 top-1/2 h-5 w-5 -translate-y-1/2 text-slate-400" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="10.8" cy="10.8" r="6.8" /><path d="m16 16 4 4" /></svg>
				<input
					id="file-search"
					bind:value={search}
					type="search"
					placeholder="Search files and folders"
					class="min-h-12 w-full rounded-xl border border-slate-200 bg-white py-3 pl-12 pr-4 text-base outline-none transition focus:border-indigo-500 focus:ring-4 focus:ring-indigo-500/10 dark:border-slate-800 dark:bg-slate-900 dark:focus:border-indigo-400"
				/>
			</div>

			<div class="flex items-center justify-between gap-3 px-1">
				<div>
					<h2 class="font-semibold">Files</h2>
					<p class="text-sm text-slate-500 dark:text-slate-400">{visibleItems.length} {visibleItems.length === 1 ? 'item' : 'items'}</p>
					<nav aria-label="Folder breadcrumb" class="mt-1 flex flex-wrap items-center gap-2 text-sm text-slate-500 dark:text-slate-400">
						<button class="rounded px-1 py-1 hover:text-indigo-600 focus-visible:outline-2 focus-visible:outline-indigo-500 dark:hover:text-indigo-300" aria-current={segments.length ? undefined : 'page'} onclick={() => go('')}>Home</button>
						{#each segments as segment, i}
							<span aria-hidden="true">/</span>
							<button class="max-w-48 truncate rounded px-1 py-1 hover:text-indigo-600 focus-visible:outline-2 focus-visible:outline-indigo-500 dark:hover:text-indigo-300" aria-current={i === segments.length - 1 ? 'page' : undefined} onclick={() => go(segments.slice(0, i + 1).join('/'))}>{segment}</button>
						{/each}
					</nav>
				</div>
				{#if segments.length}
					<button class="min-h-11 rounded-lg px-3 text-sm font-medium text-indigo-600 hover:bg-indigo-50 focus-visible:outline-2 focus-visible:outline-indigo-500 dark:text-indigo-300 dark:hover:bg-indigo-400/10" onclick={() => go(segments.slice(0, -1).join('/'))}>← Parent folder</button>
				{/if}
			</div>

			<div class="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-sm shadow-slate-900/[0.03] dark:border-slate-800 dark:bg-slate-900">
				<div class="hidden grid-cols-[minmax(0,1fr)_100px] border-b border-slate-100 bg-slate-50 px-4 py-3 text-xs font-semibold uppercase tracking-wide text-slate-500 sm:grid dark:border-slate-800 dark:bg-slate-800/60 dark:text-slate-400">
					<span>Name</span><span class="text-right">Size</span>
				</div>
				{#each visibleItems as item (item.name)}
					{#if item.dir}
						<button class="flex min-h-14 w-full items-center gap-3 border-b border-slate-100 px-4 py-2.5 text-left transition last:border-0 hover:bg-slate-50 focus-visible:relative focus-visible:outline-2 focus-visible:outline-indigo-500 dark:border-slate-800 dark:hover:bg-slate-800/70" onclick={() => go([...segments, item.name].join('/'))}>
							<span class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl {iconColor(item)}"><svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor"><path d="M3 6.75A1.75 1.75 0 0 1 4.75 5h5.1c.47 0 .92.19 1.25.52l1.23 1.23h6.92A1.75 1.75 0 0 1 21 8.5v8.75A1.75 1.75 0 0 1 19.25 19H4.75A1.75 1.75 0 0 1 3 17.25z" /></svg></span>
							<span class="min-w-0 flex-1 truncate text-sm font-medium">{item.name}</span>
							<span class="shrink-0 text-xs text-slate-500 dark:text-slate-400">Folder</span>
							<svg aria-hidden="true" viewBox="0 0 20 20" class="h-4 w-4 text-slate-400" fill="currentColor"><path fill-rule="evenodd" d="M7.21 14.77a.75.75 0 0 1 .02-1.06L10.94 10 7.23 6.29a.75.75 0 1 1 1.06-1.06l4.24 4.24a.75.75 0 0 1 0 1.06l-4.24 4.24a.75.75 0 0 1-1.08 0Z" clip-rule="evenodd" /></svg>
						</button>
					{:else}
						<a class="flex min-h-14 items-center gap-3 border-b border-slate-100 px-4 py-2.5 transition last:border-0 hover:bg-slate-50 focus-visible:relative focus-visible:outline-2 focus-visible:outline-indigo-500 dark:border-slate-800 dark:hover:bg-slate-800/70" href={href(item.name)}>
							<span class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl {iconColor(item)}">
								{#if iconKind(item) === 'image'}
									<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3.5" y="4.5" width="17" height="15" rx="2" /><circle cx="9" cy="10" r="1.5" /><path d="m4.5 17 5-4 3 2 3-3 4 4" /></svg>
								{:else if iconKind(item) === 'media'}
									<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor"><path d="M8 5.8c0-.78.85-1.25 1.5-.84l9.1 5.7a1 1 0 0 1 0 1.68l-9.1 5.7A1 1 0 0 1 8 17.2z" /></svg>
								{:else if iconKind(item) === 'code'}
									<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m8 8-4 4 4 4m8-8 4 4-4 4m-3-10-2 12" /></svg>
								{:else}
									<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M6 3.75h8l4.25 4.5v12H6z" /><path d="M14 4v5h4m-9 4h6m-6 3h6" />{#if iconKind(item) === 'pdf'}<path d="M8 18h7" stroke-width="2.5" />{/if}{#if iconKind(item) === 'sheet'}<path d="M9 12v6m4-6v6m-4-3h7" />{/if}{#if iconKind(item) === 'archive'}<path d="M11 10v2m0 2v2" stroke-width="2.5" />{/if}</svg>
								{/if}
							</span>
							<span class="min-w-0 flex-1 truncate text-sm font-medium">{item.name}</span>
							<span class="shrink-0 text-sm tabular-nums text-slate-500 dark:text-slate-400">{size(item.size)}</span>
						</a>
					{/if}
				{:else}
					<div class="px-5 py-12 text-center">
						<span class="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-slate-100 text-slate-400 dark:bg-slate-800 dark:text-slate-500"><svg aria-hidden="true" viewBox="0 0 24 24" class="h-6 w-6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.75h8l4.25 4.5v12H6zM14 4v5h4M9 13h6m-6 3h4" /></svg></span>
						<p class="font-medium">{search ? 'No matching files' : 'This folder is empty'}</p>
						<p class="mt-1 text-sm text-slate-500 dark:text-slate-400">{search ? 'Try a different name.' : 'Upload a file or drop it here to get started.'}</p>
					</div>
				{/each}
			</div>
		</section>
	</section>
</main>

{#if ask}
	<div class="fixed inset-0 flex items-center justify-center bg-black/50 p-6">
		<div class="flex w-full max-w-sm flex-col gap-4 rounded-xl bg-white p-6 text-gray-900 dark:bg-gray-900 dark:text-gray-100">
			<div>
				<p class="text-lg font-medium">{ask.peer} wants to send you</p>
				<p class="truncate text-sm text-gray-500">{ask.name} · {size(ask.size)}</p>
			</div>
			<div class="flex gap-3">
				<button
					class="flex-1 rounded-lg bg-blue-600 p-3 font-medium text-white hover:bg-blue-700"
					onclick={() => respond(ask.id, true)}
				>
					Accept
				</button>
				<button
					class="flex-1 rounded-lg border border-gray-300 p-3 font-medium dark:border-gray-700"
					onclick={() => respond(ask.id, false)}
				>
					Decline
				</button>
			</div>
		</div>
	</div>
{/if}
