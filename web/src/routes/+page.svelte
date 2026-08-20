<script lang="ts">
	type Drop = { id: number; name: string; size: number; mime: string };
	type Item = { name: string; size: number; dir: boolean };

	let files = $state<Drop[]>([]);
	let items = $state<Item[]>([]);
	let path = $state('');
	let dragging = $state(false);
	let sending = $state(0);
	let error = $state('');

	async function refresh() {
		try {
			files = await (await fetch('/_api/files')).json();
			const at = path;
			const listing = await (await fetch(`/_api/dir?path=${encodeURIComponent(at)}`)).json();
			// a poll that started before the last click must not drag the view back
			if (at === path) items = listing.entries;
		} catch {
			// the server went away; the next tick will pick it back up
		}
	}

	// ponytail: 2s polling, swap for SSE if the lists need to feel instant
	$effect(() => {
		refresh();
		const timer = setInterval(refresh, 2000);
		return () => clearInterval(timer);
	});

	const segments = $derived(path.split('/').filter(Boolean));

	function open(name: string) {
		path = [...segments, name].join('/');
		refresh();
	}

	function href(name: string) {
		return '/' + [...segments, name].map(encodeURIComponent).join('/');
	}

	async function send(list: FileList | null) {
		error = '';
		for (const file of Array.from(list ?? [])) {
			sending++;
			try {
				const res = await fetch('/_api/send', {
					method: 'POST',
					headers: {
						'x-filename': encodeURIComponent(file.name),
						'content-type': file.type || 'application/octet-stream'
					},
					body: file
				});
				if (!res.ok) {
					error = res.status === 413 ? `${file.name} is over the 100 MB limit` : `${file.name}: ${res.status}`;
				}
			} catch (e) {
				error = `${file.name}: ${e}`;
			}
			sending--;
		}
		refresh();
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

{#snippet row(left: string, right: string)}
	<span class="min-w-0 flex-1 truncate">{left}</span>
	<span class="shrink-0 text-sm text-gray-500">{right}</span>
{/snippet}

<main class="mx-auto flex min-h-screen max-w-2xl flex-col gap-6 bg-white p-6 text-gray-900 dark:bg-gray-950 dark:text-gray-100">
	<header>
		<h1 class="text-2xl font-semibold">Drop</h1>
		<p class="text-sm text-gray-500">Send a file here and every device on this server sees it.</p>
	</header>

	<label
		class="flex cursor-pointer flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed p-10 text-center transition-colors {dragging
			? 'border-blue-500 bg-blue-50 dark:bg-blue-950'
			: 'border-gray-300 hover:border-gray-400 dark:border-gray-700 dark:hover:border-gray-500'}"
		ondragover={(e) => {
			e.preventDefault();
			dragging = true;
		}}
		ondragleave={() => (dragging = false)}
		ondrop={(e) => {
			e.preventDefault();
			dragging = false;
			send(e.dataTransfer?.files ?? null);
		}}
	>
		<span class="text-lg">{sending > 0 ? `Sending ${sending}…` : 'Drop files or tap to choose'}</span>
		<span class="text-xs text-gray-500">up to 100 MB each</span>
		<input
			type="file"
			multiple
			class="hidden"
			onchange={(e) => {
				const input = e.currentTarget;
				send(input.files);
				input.value = '';
			}}
		/>
	</label>

	{#if error}
		<p class="rounded-lg bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>
	{/if}

	{#if files.length}
		<section class="flex flex-col gap-2">
			<h2 class="text-sm font-medium text-gray-500">Received</h2>
			{#each files as file (file.id)}
				<div class="flex items-center justify-between gap-4 rounded-lg border border-gray-200 p-3 dark:border-gray-800">
					{@render row(file.name, size(file.size))}
					<a class="shrink-0 text-sm font-medium text-blue-600 hover:underline dark:text-blue-400" href="/_api/files/{file.id}" download={file.name}>
						Download
					</a>
				</div>
			{/each}
		</section>
	{/if}

	<section class="flex flex-col gap-2">
		<h2 class="flex flex-wrap items-center gap-1 text-sm font-medium text-gray-500">
			<button class="hover:underline" onclick={() => ((path = ''), refresh())}>Files here</button>
			{#each segments as segment, i}
				<span>/</span>
				<button class="hover:underline" onclick={() => ((path = segments.slice(0, i + 1).join('/')), refresh())}>
					{segment}
				</button>
			{/each}
		</h2>

		{#if segments.length}
			<button
				class="rounded-lg border border-gray-200 p-3 text-left dark:border-gray-800"
				onclick={() => ((path = segments.slice(0, -1).join('/')), refresh())}
			>
				..
			</button>
		{/if}

		{#each items as item (item.name)}
			{#if item.dir}
				<button
					class="flex items-center justify-between gap-4 rounded-lg border border-gray-200 p-3 text-left hover:border-gray-400 dark:border-gray-800 dark:hover:border-gray-500"
					onclick={() => open(item.name)}
				>
					{@render row(`${item.name}/`, '')}
				</button>
			{:else}
				<a
					class="flex items-center justify-between gap-4 rounded-lg border border-gray-200 p-3 hover:border-gray-400 dark:border-gray-800 dark:hover:border-gray-500"
					href={href(item.name)}
				>
					{@render row(item.name, size(item.size))}
				</a>
			{/if}
		{:else}
			<p class="p-3 text-sm text-gray-500">This folder is empty.</p>
		{/each}
	</section>
</main>
