/* 서비스 워커: 휴대폰 알림을 띄우고, 알림을 누르면 앱을 열어요. (브라우저가 알림은 서비스 워커로만 띄우게 해서 이 파일만 JS로 둬요) */
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim()));
self.addEventListener("notificationclick", (e) => {
  e.notification.close();
  const chat = (e.notification.data && e.notification.data.chat) || "";
  e.waitUntil((async () => {
    const all = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    for (const c of all) { c.postMessage({ type: "open-chat", chat }); return c.focus(); }
    return self.clients.openWindow("./?chat=" + encodeURIComponent(chat));
  })());
});
