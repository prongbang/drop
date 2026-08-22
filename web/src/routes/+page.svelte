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

	let me = $state('');
	let myName = $state('');
	let peers = $state<Peer[]>([]);
	let transfers = $state<Transfer[]>([]);
	let items = $state<Item[]>([]);
	let path = $state('');
	let dragging = $state('');
	let error = $state('');
	let picker: HTMLInputElement;
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

	function status(transfer: Transfer) {
		if (transfer.stage === 'declined') return 'declined';
		if (transfer.stage === 'ready') return transfer.incoming ? '' : 'sent';
		if (transfer.stage === 'accepted') return 'sending…';
		return transfer.incoming ? 'waiting for you' : 'waiting for accept';
	}

	const segments = $derived(path.split('/').filter(Boolean));

	function go(next: string) {
		path = next;
		listDir();
	}

	function href(name: string) {
		return '/' + [...segments, name].map(encodeURIComponent).join('/');
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
		<h2 class="flex flex-wrap items-center gap-1 text-sm font-medium text-gray-500">
			<button class="hover:underline" onclick={() => go('')}>Files here</button>
			{#each segments as segment, i}
				<span>/</span>
				<button class="hover:underline" onclick={() => go(segments.slice(0, i + 1).join('/'))}>{segment}</button>
			{/each}
		</h2>

		{#if segments.length}
			<button
				class="rounded-lg border border-gray-200 p-3 text-left dark:border-gray-800"
				onclick={() => go(segments.slice(0, -1).join('/'))}
			>
				..
			</button>
		{/if}

		{#each items as item (item.name)}
			{#if item.dir}
				<button
					class="flex items-center justify-between gap-4 rounded-lg border border-gray-200 p-3 text-left hover:border-gray-400 dark:border-gray-800 dark:hover:border-gray-500"
					onclick={() => go([...segments, item.name].join('/'))}
				>
					<span class="min-w-0 flex-1 truncate">{item.name}/</span>
				</button>
			{:else}
				<a
					class="flex items-center justify-between gap-4 rounded-lg border border-gray-200 p-3 hover:border-gray-400 dark:border-gray-800 dark:hover:border-gray-500"
					href={href(item.name)}
				>
					<span class="min-w-0 flex-1 truncate">{item.name}</span>
					<span class="shrink-0 text-sm text-gray-500">{size(item.size)}</span>
				</a>
			{/if}
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
