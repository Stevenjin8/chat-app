NAME=localhost:5001/chat-app:latest
dbuild:
	docker build . -t $(NAME)

drun:
	docker run $(NAME)

dpush:
	docker push $(NAME)


.PHONY: dbuild drun dpush
