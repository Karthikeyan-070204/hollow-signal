"""Unix PTY smoke check: build release first, then python3 scripts/terminal_smoke.py."""
import os,pty,fcntl,termios,struct,subprocess,time,select,socket,json
from pathlib import Path
binary=str(Path(__file__).resolve().parents[1] / 'target' / 'release' / 'hollow-signal')
s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close()
items=[]
def start(args):
 master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,120,0,0))
 env=dict(os.environ,TERM='xterm-256color',COLORTERM='truecolor')
 proc=subprocess.Popen([binary]+args,stdin=slave,stdout=slave,stderr=slave,start_new_session=True,env=env)
 entry={'master':master,'slave':slave,'proc':proc,'data':bytearray()};items.append(entry);return entry
def pump(seconds):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  ready,_,_=select.select([e['master'] for e in items],[],[],0.04)
  for e in items:
   if e['master'] in ready:
    try:e['data'].extend(os.read(e['master'],65536))
    except OSError:pass
try:
 host=start(['host','--bind',f'127.0.0.1:{port}','--name','HostCheck','--color','cyan']);pump(2.5)
 client=start(['join',f'127.0.0.1:{port}','--name','FriendCheck','--color','violet']);pump(2.5)
 os.write(host['master'],b'\t');os.write(client['master'],b'\t');pump(0.5)
 for e in items:
  assert b'HostCheck' in e['data'] and b'FriendCheck' in e['data'],'both names must be rendered'
  assert b'\x1b[?1049h' in e['data'],'alternate screen must be entered'
  assert b'38;2;' in e['data'] or b'38:2' in e['data'],'truecolour frame must be rendered'
 os.write(client['master'],b'\x1b');pump(0.5)
 assert client['proc'].poll()==0,'client must exit cleanly'
 os.write(host['master'],b'\twh');pump(0.4)
 assert b'FIELD MANUAL' in host['data'],'help must open'
 os.write(host['master'],b'\x1b');pump(0.5)
 assert host['proc'].poll()==0,'host must exit cleanly'
 for e in items:
  assert b'\x1b[?1049l' in e['data'],'alternate screen must be left'
  flags=termios.tcgetattr(e['slave'])[3]
  assert flags & termios.ICANON and flags & termios.ECHO,'terminal must leave raw mode'
 print(json.dumps({'result':'PASS','real_processes':2,'checks':['host-and-play launch','second client join','both names in both terminals','RGB frames','scoreboard','help','Escape exits','alternate screen restored','raw mode restored'],'bytes_rendered':[len(e['data']) for e in items]}))
finally:
 for e in items:
  if e['proc'].poll() is None:e['proc'].terminate();e['proc'].wait(timeout=3)
  os.close(e['master']);os.close(e['slave'])
