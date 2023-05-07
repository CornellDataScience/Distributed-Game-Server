import json
import socket
import time
import pickle
import requests

def ip():
    try:
        IP = socket.gethostbyname(socket.gethostname())
        print("Game server IP:", IP)
    except Exception:
        IP = '127.0.0.1'
    return IP

class Server:
    def __init__(self) -> None:
        self.status = True
        self.port = 60000

    def session(self, conns, peers):
        while self.status:
            # if not snake1 or not snake2:
            #     break
            recvm = []
            snake_data = []
            for i in range(self.players):
                recvm.append(conns[i].recv(1024))
            for i in range(self.players):
                snake = pickle.loads(recvm[i])
                snake_data.append(snake)
            self.to_raft(snake_data, peers)
            for i in range(len(conns)):
                conns[i].sendall(pickle.dumps(snake_data[:i]+snake_data[i+1:]))

    def start(self):
        self.status = True
        self.s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.s.bind((ip(), self.port))
        self.s.listen(5)
        self.players = 2
        print("Game server running...")
        while self.status:
            conns = []
            # waits for player connections to server
            for _ in range(self.players):
                conn, addr = self.s.accept()
                print("Connected to server: ", addr)
                conns.append(conn)
            print(conns)
            time.sleep(3)
            peers = requests.get("http://127.0.0.1:8000/get-peers").json()
            self.session(conns, peers)
            # ts = threading.Thread(target=self.session, args = (conns,))
            # ts.start()


    def stop(self):
        self.running = False

    def to_raft(self, data, peers):
        headers = {'Content-type':'application/json', 'Accept':'application/json'}
        requests.post("http://127.0.0.1:8000/put-info",json={"data":data}, headers=headers)


srvr = Server()
srvr.start()