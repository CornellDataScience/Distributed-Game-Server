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
        self.grid_size = 50
        self.display_size = 600
        self.snake = self.spawn()
        self.dir = "up"
        self.blue = (0, 0, 255)
        self.red = (255, 0, 0)
        self.endgame = False
        pygame.init()
        self.display = pygame.display.set_mode((self.display_size,self.display_size))
        self.loop()

    def spawn(self):
        loc = []
        x = random.randint(30,35)
        y = random.randint(5,45)
        for i in range(10):
            loc.append([x-i, y])
        return loc[::-1]


    def draw_snake(self, snake, color):
        ratio = self.display_size // self.grid_size
        for x,y in snake:
            pygame.draw.rect(self.display,color,(ratio*x,ratio*y,ratio,ratio))


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
    
    def check_collision(self, snake, enemies):
        head = snake[-1]
        if head[0] < 0 or head[0] >= self.grid_size or head[1] < 0 or head[1] >= self.grid_size:
            return True
        for i, loc in enumerate(snake):
            if head == loc and i != len(snake)-1: return True
        for enemy in enemies:
            for loc in enemy:
                if head == loc: return True
        return False

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
                if not self.endgame: self.move_snake()
                self.draw_snake(self.snake, self.blue)
                self.sc.send(self.snake)
                self.enemies = self.sc.recv()
                for e in self.enemies:
                    self.draw_snake(e, self.red)
                # print(self.enemy)
                if self.check_collision(self.snake, self.enemies): 
                    self.endgame = True
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
