package main
import("fmt";"net";"net/http";"net/http/httptest";"strings";"sync";"time")
func main(){
 var mu sync.Mutex;var s *httptest.Server
 start:=func(){
  srv:=httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter,r *http.Request){
   w.Header().Set("Content-Type","application/octet-stream");w.Header().Set("X-AB-Protocol",r.Proto)
   if r.URL.Path=="/slow" {time.Sleep(100*time.Millisecond)}
   if r.URL.Path=="/stream" {for i:=0;i<4;i++ {fmt.Fprint(w,strings.Repeat("s",4096));w.(http.Flusher).Flush();time.Sleep(20*time.Millisecond)};return}
   fmt.Fprint(w,strings.Repeat("p",65536))
  }));srv.Listener.Close();l,err:=net.Listen("tcp","127.0.0.1:28081");if err!=nil{panic(err)};srv.Listener=l;srv.EnableHTTP2=true;srv.StartTLS();s=srv
 };start()
 http.HandleFunc("/control",func(w http.ResponseWriter,r *http.Request){mu.Lock();defer mu.Unlock();if r.URL.Query().Get("mode")=="off" {if s!=nil{s.CloseClientConnections();s.Close();s=nil}}else if s==nil{start()};fmt.Fprint(w,"ok")})
 panic(http.ListenAndServe("127.0.0.1:28082",nil))
}
