#! /bin/bash
docker build --tag elm-builder elm
docker run --rm -d --name elmb elm-builder 
docker cp $PWD/elm/src elmb:/workdir
docker exec -i elmb /elm/elm make --output=src/main.js src/Main.elm
docker cp elmb:/workdir/src/main.js $PWD/static
docker rm -f elmb

if [ ! -f "$PWD/static/main.js" ];then
exit 125
fi