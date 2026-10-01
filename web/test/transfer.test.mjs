import { describe, expect, test } from 'bun:test';
import { chunkOffsets, progressPercent, sendBlobChunks, uploadBlobWithProgress } from '../src/lib/transfer.ts';

class FakeChannel extends EventTarget {
	readyState = 'open';
	bufferedAmount = 0;
	bufferedAmountLowThreshold = 0;
	sent = [];

	send(data) {
		this.sent.push(data);
		this.bufferedAmount += data.byteLength;
	}

	releaseBuffer() {
		this.bufferedAmount = 0;
		this.dispatchEvent(new Event('bufferedamountlow'));
	}
}

describe('file transfer chunks', () => {
	test('clamps visible transfer progress to a percentage', () => {
		expect(progressPercent(1, 3)).toBe(33);
		expect(progressPercent(12, 10)).toBe(100);
		expect(progressPercent(4, 0)).toBe(0);
	});

	test('reports relay upload progress and resolves after the server accepts it', async () => {
		const original = globalThis.XMLHttpRequest;
		let request;
		globalThis.XMLHttpRequest = class {
			upload = {};
			status = 0;
			open(method, url) { this.method = method; this.url = url; }
			setRequestHeader(name, value) { this.header = [name, value]; }
			send(body) { this.body = body; request = this; }
		};
		try {
			const progress = [];
			const file = new Blob(['payload'], { type: 'application/octet-stream' });
			const sending = uploadBlobWithProgress(file, '/api/upload/1', (sent, total) => progress.push([sent, total]));
			request.upload.onprogress({ lengthComputable: true, loaded: 4, total: 7 });
			request.status = 201;
			request.onload();
			await sending;
			expect(request.method).toBe('POST');
			expect(request.body).toBe(file);
			expect(progress).toEqual([[4, 7], [7, 7]]);
		} finally {
			globalThis.XMLHttpRequest = original;
		}
	});

	test('creates bounded contiguous offsets and handles an empty file', () => {
		expect(chunkOffsets(0, 4)).toEqual([]);
		expect(chunkOffsets(10, 4)).toEqual([
			{ start: 0, end: 4 },
			{ start: 4, end: 8 },
			{ start: 8, end: 10 }
		]);
	});

	test('sends the complete file in order and reports sent bytes', async () => {
		const channel = new FakeChannel();
		const progress = [];

		await sendBlobChunks(new Blob(['abcdefghij']), channel, (sent) => progress.push(sent), 4);

		expect(channel.sent.map((chunk) => new TextDecoder().decode(chunk))).toEqual(['abcd', 'efgh', 'ij']);
		expect(progress).toEqual([4, 8, 10]);
	});

	test('waits for the channel buffer to drain before sending more', async () => {
		const channel = new FakeChannel();
		channel.bufferedAmount = 8 * 1024 * 1024;
		const sending = sendBlobChunks(new Blob(['data']), channel, () => {}, 4);

		await Promise.resolve();
		expect(channel.sent).toHaveLength(0);
		channel.releaseBuffer();
		await sending;
		expect(channel.sent).toHaveLength(1);
	});

	test('rejects if the peer channel closes while waiting for buffer space', async () => {
		const channel = new FakeChannel();
		channel.bufferedAmount = 8 * 1024 * 1024;
		const sending = sendBlobChunks(new Blob(['data']), channel, () => {}, 4);
		await Promise.resolve();
		channel.readyState = 'closed';
		channel.dispatchEvent(new Event('close'));
		await expect(sending).rejects.toThrow('Data channel closed');
	});
});
