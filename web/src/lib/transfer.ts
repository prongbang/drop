export const DEFAULT_CHUNK_SIZE = 64 * 1024;
const HIGH_WATER_MARK = 8 * 1024 * 1024;
const LOW_WATER_MARK = 4 * 1024 * 1024;
const READ_SIZE = 1024 * 1024;

export function chunkOffsets(fileSize: number, chunkSize = DEFAULT_CHUNK_SIZE) {
	if (!Number.isSafeInteger(fileSize) || fileSize < 0) throw new RangeError('Invalid file size');
	if (!Number.isSafeInteger(chunkSize) || chunkSize <= 0) throw new RangeError('Invalid chunk size');
	const chunks: Array<{ start: number; end: number }> = [];
	for (let start = 0; start < fileSize; start += chunkSize) {
		chunks.push({ start, end: Math.min(start + chunkSize, fileSize) });
	}
	return chunks;
}

export function progressPercent(sent: number, total: number) {
	if (!Number.isFinite(total) || total <= 0) return 0;
	return Math.floor((Math.max(0, Math.min(sent, total)) / total) * 100);
}

export function uploadBlobWithProgress(file: Blob, url: string, onProgress: (sent: number, total: number) => void) {
	return new Promise<void>((resolve, reject) => {
		const request = new XMLHttpRequest();
		request.open('POST', url);
		if (file.type) request.setRequestHeader('content-type', file.type);
		request.upload.onprogress = (event) => {
			if (event.lengthComputable) onProgress(event.loaded, event.total);
		};
		request.onload = () => {
			if (request.status >= 200 && request.status < 300) {
				onProgress(file.size, file.size);
				resolve();
			} else reject(new Error(`Upload failed (HTTP ${request.status})`));
		};
		request.onerror = () => reject(new Error('Upload failed. Check your network and try again.'));
		request.onabort = () => reject(new Error('Upload was cancelled'));
		request.send(file);
	});
}

type BinaryChannel = Pick<
	RTCDataChannel,
	'readyState' | 'bufferedAmount' | 'bufferedAmountLowThreshold' | 'addEventListener' | 'removeEventListener' | 'send'
>;

function waitForBuffer(channel: BinaryChannel) {
	if (channel.readyState !== 'open') return Promise.reject(new Error('Data channel closed'));
	if (channel.bufferedAmount < HIGH_WATER_MARK) return Promise.resolve();

	return new Promise<void>((resolve, reject) => {
		const cleanup = () => {
			channel.removeEventListener('bufferedamountlow', onLow);
			channel.removeEventListener('close', onClose);
		};
		const onLow = () => {
			cleanup();
			channel.readyState === 'open' ? resolve() : reject(new Error('Data channel closed'));
		};
		const onClose = () => {
			cleanup();
			reject(new Error('Data channel closed'));
		};

		channel.bufferedAmountLowThreshold = LOW_WATER_MARK;
		channel.addEventListener('bufferedamountlow', onLow);
		channel.addEventListener('close', onClose);
		// The buffer can drain between the initial check and listener registration.
		if (channel.bufferedAmount < HIGH_WATER_MARK) onLow();
		else if (channel.readyState !== 'open') onClose();
	});
}

export async function sendBlobChunks(
	file: Blob,
	channel: BinaryChannel,
	onProgress: (sentBytes: number) => void,
	chunkSize = DEFAULT_CHUNK_SIZE
) {
	channel.bufferedAmountLowThreshold = LOW_WATER_MARK;
	let sent = 0;
	for (let blockStart = 0; blockStart < file.size; blockStart += READ_SIZE) {
		const blockEnd = Math.min(blockStart + READ_SIZE, file.size);
		const block = await file.slice(blockStart, blockEnd).arrayBuffer();
		for (let offset = 0; offset < block.byteLength; offset += chunkSize) {
			await waitForBuffer(channel);
			const length = Math.min(chunkSize, block.byteLength - offset);
			if (channel.readyState !== 'open') throw new Error('Data channel closed');
			channel.send(new Uint8Array(block, offset, length));
			sent += length;
			onProgress(sent);
		}
	}
}
