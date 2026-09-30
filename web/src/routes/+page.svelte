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
	let error = $state('');
	let picker: HTMLInputElement;
	let uploadPicker: HTMLInputElement;
	let uploading = $state(false);
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

	async function listDir() {
		try {
			const at = path;
			const listing = await (await fetch(`/api/dir?path=${encodeURIComponent(at)}`)).json();
			// a poll that started before the last click must not drag the view back
			if (at === path) items = listing.entries;
		} catch {
			// the server went away; the next tick will pick it back up
		}
	}

	$effect(() => {
		me = localStorage.getItem('peer') ?? crypto.randomUUID();
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
		uploading = true;
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
		} finally {
			uploading = false;
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

	function go(next: string) {
		path = next;
		listDir();
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

<main class="mx-auto flex min-h-screen max-w-2xl flex-col gap-6 bg-white p-6 text-gray-900 dark:bg-gray-950 dark:text-gray-100">
	<header>
		<h1 class="text-2xl font-semibold">Drop</h1>
		<p class="text-sm text-gray-500">You are {myName || '…'}</p>
	</header>

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

	<section class="flex flex-col gap-2">
		<h2 class="text-sm font-medium text-gray-500">Devices here</h2>
		{#each peers as peer (peer.id)}
			<button
				class="flex items-center justify-between gap-4 rounded-lg border-2 border-dashed p-4 text-left transition-colors {dragging ===
				peer.id
					? 'border-blue-500 bg-blue-50 dark:bg-blue-950'
					: 'border-gray-300 hover:border-gray-400 dark:border-gray-700 dark:hover:border-gray-500'}"
				onclick={() => {
					target = peer.id;
					picker.click();
				}}
				ondragover={(e) => {
					e.preventDefault();
					dragging = peer.id;
				}}
				ondragleave={() => (dragging = '')}
				ondrop={(e) => {
					e.preventDefault();
					dragging = '';
					offer(peer.id, e.dataTransfer?.files ?? null);
				}}
			>
				<span class="min-w-0 flex-1 truncate">{peer.name}</span>
				<span class="shrink-0 text-sm text-gray-500">drop or tap to send</span>
			</button>
		{:else}
			<p class="p-3 text-sm text-gray-500">No one else is here yet. Scan the QR code on another device.</p>
		{/each}
	</section>

	{#if error}
		<p class="rounded-lg bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>
	{/if}

	{#if transfers.length}
		<section class="flex flex-col gap-2">
			<h2 class="text-sm font-medium text-gray-500">Transfers</h2>
			{#each transfers as transfer (transfer.id)}
				<div class="flex items-center justify-between gap-4 rounded-lg border border-gray-200 p-3 dark:border-gray-800">
					<span class="min-w-0 flex-1 truncate">
						{transfer.name}
						<span class="text-sm text-gray-500">
							{transfer.incoming ? 'from' : 'to'}
							{transfer.peer}
						</span>
					</span>
					<span class="shrink-0 text-sm text-gray-500">{size(transfer.size)} {status(transfer)}</span>
					{#if transfer.incoming && transfer.stage === 'ready'}
						<a
							class="shrink-0 text-sm font-medium text-blue-600 hover:underline dark:text-blue-400"
							href="/api/transfer/{transfer.id}?me={me}"
							download={transfer.name}
						>
							Download
						</a>
					{/if}
				</div>
			{/each}
		</section>
	{/if}

	<section class="flex flex-col gap-2">
		<input
			bind:value={search}
			type="search"
			placeholder="Search files and folders"
			aria-label="Search files and folders"
			class="w-full rounded-lg border border-gray-300 bg-white px-4 py-3 text-sm outline-none focus:border-blue-500 dark:border-gray-700 dark:bg-gray-900"
		/>
		<div class="flex flex-wrap items-center justify-between gap-3">
			<h2 class="flex flex-wrap items-center gap-1 text-sm font-medium text-gray-500">
				<button class="hover:underline" onclick={() => go('')}>Files here</button>
				{#each segments as segment, i}
					<span>/</span>
					<button class="hover:underline" onclick={() => go(segments.slice(0, i + 1).join('/'))}>{segment}</button>
				{/each}
			</h2>
			<button
				class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-wait disabled:opacity-60"
				disabled={uploading}
				onclick={() => uploadPicker.click()}
			>
				{uploading ? 'Uploading…' : 'Upload files'}
			</button>
		</div>

		{#if segments.length}
			<button
				class="rounded-lg border border-gray-200 p-3 text-left dark:border-gray-800"
				onclick={() => go(segments.slice(0, -1).join('/'))}
			>
				..
			</button>
		{/if}

		{#each visibleItems as item (item.name)}
			{#if item.dir}
				<button
					class="group flex items-center gap-3 rounded-lg px-3 py-2 text-left hover:bg-gray-100 dark:hover:bg-gray-900"
					onclick={() => go([...segments, item.name].join('/'))}
				>
					<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg {iconColor(item)}">
						<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor">
							<path d="M3 6.75A1.75 1.75 0 0 1 4.75 5h5.1c.47 0 .92.19 1.25.52l1.23 1.23h6.92A1.75 1.75 0 0 1 21 8.5v8.75A1.75 1.75 0 0 1 19.25 19H4.75A1.75 1.75 0 0 1 3 17.25z" />
						</svg>
					</span>
					<span class="min-w-0 flex-1 truncate text-sm font-medium">{item.name}</span>
					<svg aria-hidden="true" viewBox="0 0 20 20" class="h-4 w-4 text-gray-400" fill="currentColor">
						<path fill-rule="evenodd" d="M7.21 14.77a.75.75 0 0 1 .02-1.06L10.94 10 7.23 6.29a.75.75 0 1 1 1.06-1.06l4.24 4.24a.75.75 0 0 1 0 1.06l-4.24 4.24a.75.75 0 0 1-1.08 0Z" clip-rule="evenodd" />
					</svg>
				</button>
			{:else}
				<a
					class="group flex items-center gap-3 rounded-lg px-3 py-2 hover:bg-gray-100 dark:hover:bg-gray-900"
					href={href(item.name)}
				>
					<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg {iconColor(item)}">
						{#if iconKind(item) === 'image'}
							<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8">
								<rect x="3.5" y="4.5" width="17" height="15" rx="2" />
								<circle cx="9" cy="10" r="1.5" />
								<path d="m4.5 17 5-4 3 2 3-3 4 4" />
							</svg>
						{:else if iconKind(item) === 'media'}
							<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor">
								<path d="M8 5.8c0-.78.85-1.25 1.5-.84l9.1 5.7a1 1 0 0 1 0 1.68l-9.1 5.7A1 1 0 0 1 8 17.2z" />
							</svg>
						{:else if iconKind(item) === 'code'}
							<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
								<path d="m8 8-4 4 4 4m8-8 4 4-4 4m-3-10-2 12" />
							</svg>
						{:else}
							<svg aria-hidden="true" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round">
								<path d="M6 3.75h8l4.25 4.5v12H6z" />
								<path d="M14 4v5h4m-9 4h6m-6 3h6" />
								{#if iconKind(item) === 'pdf'}<path d="M8 18h7" stroke-width="2.5" />{/if}
								{#if iconKind(item) === 'sheet'}<path d="M9 12v6m4-6v6m-4-3h7" />{/if}
								{#if iconKind(item) === 'archive'}<path d="M11 10v2m0 2v2" stroke-width="2.5" />{/if}
							</svg>
						{/if}
					</span>
					<span class="min-w-0 flex-1 truncate text-sm">{item.name}</span>
					<span class="shrink-0 text-xs tabular-nums text-gray-500">{size(item.size)}</span>
				</a>
			{/if}
		{:else}
			<p class="rounded-lg border border-dashed border-gray-300 p-4 text-sm text-gray-500 dark:border-gray-700">
				{search ? 'No matching files or folders.' : 'This folder is empty.'}
			</p>
		{/each}
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
