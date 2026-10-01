import { describe, expect, test } from 'bun:test';
import { progressPercent, uploadBlobWithProgress } from '../src/lib/transfer.ts';

describe('file upload progress', () => {
	test('clamps visible progress to a percentage', () => {
		expect(progressPercent(1, 3)).toBe(33);
		expect(progressPercent(12, 10)).toBe(100);
		expect(progressPercent(4, 0)).toBe(0);
	});

	test('reports upload progress and completes when the server accepts the file', async () => {
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
			expect(request.header).toEqual(['content-type', 'application/octet-stream']);
			expect(progress).toEqual([[4, 7], [7, 7]]);
		} finally {
			globalThis.XMLHttpRequest = original;
		}
	});
});
