export function configureSafeNeteaseApiCache(apiCache) {
  // 上游 apicache 在当前嵌入方式里只按 pathname 命中，query/body 不同也会串响应。
  // 与其把登录参数塞进内存 key，不如关闭这层短缓存；业务层已有自己的安全缓存。
  apiCache.options({ enabled: false });
}

export function findMatchingAudioResponse(responses, trackID) {
  if (!Array.isArray(responses)) return null;
  const expectedID = Number(trackID);
  return (
    responses.find(response => Number(response?.id) === expectedID) || null
  );
}

export function isTrustedTrackSource(record, requestedTrackID) {
  if (!record) return false;
  const expectedID = Number(requestedTrackID);
  return (
    Number(record.id) === expectedID &&
    Number(record.validatedTrackID) === expectedID
  );
}
