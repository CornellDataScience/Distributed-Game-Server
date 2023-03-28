import socket
import threading
import sys
import pickle

def ip():
    try:
        IP = socket.gethostbyname(socket.gethostname())
        IP = '127.0.0.1'
    except Exception:
        IP = '127.0.0.1'
    return IP

class Server:
    def __init__(self) -> None:
        self.status = True
        self.port = 60000

    def session(self, conns):
        snake1, snake2 = [],[]
        while self.status:
            snake1 = conns[0].recv(1024)
            snake2 = conns[1].recv(1024)
            print(snake1,snake2)
            if not snake1 or not snake2:
                break

            snake1 = pickle.loads(snake1)
            snake2 = pickle.loads(snake2)
            snake1 = pickle.dumps(snake1)
            snake2 = pickle.dumps(snake2)

            conns[0].sendall(snake2)
            conns[1].sendall(snake1)

    def start(self):
        self.status = True
        self.s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.s.bind((ip(), self.port))
        self.s.listen(5)
        print("Game server running...")
        while self.status:
            conns = []
            # waits for 4 connections to server
            for _ in range(2):
                conn, addr = self.s.accept()
                print("Connected to server: ", addr)
                conns.append(conn)
            print(conns)
            self.session(conns)
            # ts = threading.Thread(target=self.session, args = (conns,))
            # ts.start()


    def stop(self):
        self.running = False


srvr = Server()
srvr.start()