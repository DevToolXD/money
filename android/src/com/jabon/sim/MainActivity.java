package com.jabon.sim;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.PermissionRequest;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

/**
 * 자본주의 시뮬레이터 앱 껍데기: 사이트(https://www.xn--sn3bu3q0e19c.com/)를 화면 가득 보여 주는 WebView 하나예요.
 * 사이트는 이 앱을 "JabonApp"이라는 이름표(User-Agent 끝)로 알아봐서 설치 안내를 띄우지 않아요.
 * 사진 고르기(<input type=file>)와 음성 메모(마이크)만 앱 쪽에서 이어 줘요.
 */
public class MainActivity extends Activity {
    private static final String HOST = "www.xn--sn3bu3q0e19c.com";
    private static final String HOME = "https://" + HOST + "/";
    private static final int REQ_FILE = 1, REQ_MIC = 2;

    private WebView web;
    private ValueCallback<Uri[]> fileCb;
    private PermissionRequest micReq;

    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        getWindow().setStatusBarColor(Color.parseColor("#f2f2f7"));
        getWindow().setNavigationBarColor(Color.parseColor("#f2f2f7"));
        if (Build.VERSION.SDK_INT >= 26) {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
        } else {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR);
        }
        web = new WebView(this);
        web.setLayoutParams(new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        web.setBackgroundColor(Color.parseColor("#f2f2f7"));
        setContentView(web);

        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setDatabaseEnabled(true);
        s.setMediaPlaybackRequiresUserGesture(false);
        s.setAllowFileAccess(false);
        s.setAllowContentAccess(false);
        s.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        s.setSupportZoom(false);
        s.setBuiltInZoomControls(false);
        s.setUserAgentString(s.getUserAgentString() + " JabonApp/1");

        web.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest r) {
                Uri u = r.getUrl();
                if ("https".equals(u.getScheme()) && HOST.equals(u.getHost())) return false;
                try {
                    startActivity(new Intent(Intent.ACTION_VIEW, u));
                } catch (Exception e) {
                    // 열 앱이 없으면 그냥 둬요
                }
                return true;
            }

            @Override
            public void onReceivedError(WebView v, WebResourceRequest r, WebResourceError e) {
                if (!r.isForMainFrame()) return;
                String html = "<meta name=viewport content='width=device-width,initial-scale=1'>"
                        + "<body style='margin:0;display:flex;min-height:100vh;align-items:center;justify-content:center;background:#f2f2f7;font-family:sans-serif;text-align:center'>"
                        + "<div><h2 style='margin:0 0 8px'>인터넷에 연결되지 않았어요</h2>"
                        + "<p style='color:#6e6e73;margin:0 0 20px'>연결을 확인하고 다시 해 주세요</p>"
                        + "<a href='" + HOME + "' style='display:inline-block;padding:14px 28px;border-radius:999px;background:#0a7cff;color:#fff;text-decoration:none;font-weight:700'>다시 열기</a></div></body>";
                v.loadDataWithBaseURL(HOME, html, "text/html", "UTF-8", null);
            }
        });

        web.setWebChromeClient(new WebChromeClient() {
            @Override
            public boolean onShowFileChooser(WebView v, ValueCallback<Uri[]> cb, FileChooserParams p) {
                if (fileCb != null) fileCb.onReceiveValue(null);
                fileCb = cb;
                try {
                    startActivityForResult(p.createIntent(), REQ_FILE);
                } catch (Exception e) {
                    fileCb = null;
                    return false;
                }
                return true;
            }

            @Override
            public void onPermissionRequest(PermissionRequest req) {
                boolean mic = false;
                for (String r : req.getResources()) if (PermissionRequest.RESOURCE_AUDIO_CAPTURE.equals(r)) mic = true;
                if (!mic || !HOST.equals(req.getOrigin().getHost())) {
                    req.deny();
                    return;
                }
                if (Build.VERSION.SDK_INT < 23 || checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
                    req.grant(new String[] { PermissionRequest.RESOURCE_AUDIO_CAPTURE });
                } else {
                    micReq = req;
                    requestPermissions(new String[] { Manifest.permission.RECORD_AUDIO }, REQ_MIC);
                }
            }
        });

        if (b != null) web.restoreState(b);
        else web.loadUrl(HOME);
    }

    @Override
    protected void onSaveInstanceState(Bundle b) {
        super.onSaveInstanceState(b);
        web.saveState(b);
    }

    @Override
    protected void onActivityResult(int code, int result, Intent data) {
        if (code != REQ_FILE) {
            super.onActivityResult(code, result, data);
            return;
        }
        if (fileCb != null) {
            fileCb.onReceiveValue(result == RESULT_OK ? WebChromeClient.FileChooserParams.parseResult(result, data) : null);
            fileCb = null;
        }
    }

    @Override
    public void onRequestPermissionsResult(int code, String[] perms, int[] res) {
        if (code != REQ_MIC || micReq == null) return;
        if (res.length > 0 && res[0] == PackageManager.PERMISSION_GRANTED) micReq.grant(new String[] { PermissionRequest.RESOURCE_AUDIO_CAPTURE });
        else micReq.deny();
        micReq = null;
    }

    @Override
    public void onBackPressed() {
        if (web.canGoBack()) web.goBack();
        else super.onBackPressed();
    }

    @Override
    protected void onPause() {
        super.onPause();
        web.onPause();
    }

    @Override
    protected void onResume() {
        super.onResume();
        web.onResume();
    }
}
