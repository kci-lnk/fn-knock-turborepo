import socket,struct,pathlib
state=pathlib.Path('/tmp/fn-knock-ab-20260918/dns-fail')
s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);s.bind(('127.0.0.1',53))
while True:
 raw,addr=s.recvfrom(4096)
 try:
  pos=12;parts=[]
  while raw[pos]: n=raw[pos];parts.append(raw[pos+1:pos+1+n]);pos+=1+n
  pos+=1;kind,cl=struct.unpack('!HH',raw[pos:pos+4]);question=raw[12:pos+4]
  known=b'.'.join(parts)==b'ab-upstream.test';fail=known and state.exists();answer=b''
  if known and not fail and kind==1:answer=b'\xc0\x0c'+struct.pack('!HHIH',1,1,1,4)+socket.inet_aton('127.0.0.1')
  flags=0x8182 if fail else (0x8180 if known else 0x8183)
  msg=raw[:2]+struct.pack('!HHHHH',flags,1,1 if answer else 0,0,0)+question+answer;s.sendto(msg,addr)
 except Exception:pass
