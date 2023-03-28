import socket
import pickle
import time
import pygame
import random

class Client:
    def __init__(self,host='127.0.0.1', port=60000) -> None:
        self.host = host
        self.port = port
        self.s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    def connect(self):
        self.s.connect((self.host, self.port))

    def send(self, snake):
        self.s.sendall(pickle.dumps(snake))
    
    def recv(self):
        return pickle.loads(self.s.recv(1024))

class Snake:
    def __init__(self, host='127.0.0.1', port=60000) -> None:
        super().__init__()
        self.sc = Client(host, port)
        self.sc.connect()
        self.running = True
        self.grid = [["." for j in range(50)] for i in range(50)]
        self.snake = self.spawn()
        self.dir = "up"
        self.blue = (0, 0, 255)
        self.red = (255, 0, 0)
        pygame.init()
        self.display = pygame.display.set_mode((600,600))
        self.loop()

    def spawn(self):
        loc = []
        x = random.randint(30,35)
        y = random.randint(5,45)
        for i in range(10):
            loc.append([x-i, y])
        return loc[::-1]


    def draw_snake(self, snake, color):
        for x,y in snake:
            pygame.draw.rect(self.display,color,(12*x,12*y,12,12))


    def move_snake(self):
        for i in range(len(self.snake)):
            try:
                self.snake[i] = [self.snake[i+1][0], self.snake[i+1][1]]
            except:
                if self.dir == "down":
                    self.snake[i][1]+=1
                if self.dir == "up":
                    self.snake[i][1]-=1
                if self.dir == "left":
                    self.snake[i][0]-=1
                if self.dir == "right":
                    self.snake[i][0]+=1

    def loop(self):
        fps = 10   
        t = time.time()
        while self.running:
            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    self.running = False
                self.change_direction()
            
            if time.time()-t >= 1/fps:
                self.display.fill((0, 0, 0))
                # print(self.snake)
                self.move_snake()
                self.draw_snake(self.snake, self.blue)
                self.sc.send(self.snake)
                self.enemy = self.sc.recv()
                self.draw_snake(self.enemy, self.red)
                # print(self.enemy)
                pygame.display.flip()
                t = time.time()

    def change_direction(self):
        key = pygame.key.get_pressed()
        if key[pygame.K_UP] and self.dir != "down":
            self.dir = "up"
        elif key[pygame.K_DOWN] and self.dir != "up":
            self.dir = "down"
        elif key[pygame.K_RIGHT] and self.dir != "left":
            self.dir = "right"
        elif key[pygame.K_LEFT] and self.dir != "right":
            self.dir = "left"
        

snake = Snake()
