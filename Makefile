NAME=localhost:5001/chat-app:latest
dbuild:
	docker build . -t $(NAME)

drun:
	docker run $(NAME) -p 8081:8081

dpush:
	docker push $(NAME)

dkill:
	docker kill $(NAME)



.PHONY: dbuild drun dpush dkill
