export function progressPercent(sent: number, total: number) {
	if (!Number.isFinite(total) || total <= 0) return 0;
	return Math.floor((Math.max(0, Math.min(sent, total)) / total) * 100);
}

export function uploadBlobWithProgress(file: Blob, url: string, onProgress: (sent: number, total: number) => void) {
	return new Promise<void>((resolve, reject) => {
		const request = new XMLHttpRequest();
		request.open('POST', url);
		request.setRequestHeader('content-type', file.type || 'application/octet-stream');
		request.upload.onprogress = (event) => {
			if (event.lengthComputable) onProgress(event.loaded, event.total);
		};
		request.onload = () => {
			if (request.status >= 200 && request.status < 300) {
				onProgress(file.size, file.size);
				resolve();
			} else {
				const detail = request.responseText?.trim();
				reject(new Error(`Upload failed (HTTP ${request.status})${detail ? `: ${detail}` : ''}`));
			}
		};
		request.onerror = () => reject(new Error('Upload failed. Check your network and try again.'));
		request.onabort = () => reject(new Error('Upload was cancelled'));
		request.send(file);
	});
}
