FROM fedora
RUN mkdir workdir
WORKDIR /workdir
RUN dnf update -y && dnf install wget -y
WORKDIR /
RUN mkdir elm
WORKDIR /elm
RUN wget -O elm.gz "https://github.com/elm/compiler/releases/download/0.19.1/binary-for-linux-64-bit.gz"
RUN gunzip elm.gz
RUN chmod +x elm
WORKDIR /workdir
RUN mkdir src
RUN yes | /elm/elm init 
RUN yes | /elm/elm install elm/core
RUN yes | /elm/elm install elm/html
RUN yes | /elm/elm install elm/http 
RUN yes | /elm/elm install elm/json
RUN yes | /elm/elm install elm/browser
RUN yes | /elm/elm install elm/url
RUN yes | /elm/elm install elm/time
RUN yes | /elm/elm install justinmimbs/date
RUN yes | /elm/elm install NoRedInk/elm-json-decode-pipeline
RUN yes | /elm/elm install cuducos/elm-format-number
RUN yes | /elm/elm install billstclair/elm-sortable-table
RUN yes | /elm/elm install truqu/elm-oauth2
RUN yes | /elm/elm install lovasoa/elm-csv
RUN yes | /elm/elm install rtfeldman/elm-iso8601-date-strings
RUN yes | /elm/elm install elm/parser
RUN yes | /elm/elm install truqu/elm-base64
RUN yes | /elm/elm install simonh1000/elm-jwt
RUN yes | /elm/elm install elm/file
RUN yes | /elm/elm install krisajenkins/remotedata
RUN yes | /elm/elm install elm-community/list-extra
RUN yes | /elm/elm install elm/bytes
RUN yes | /elm/elm install commonmind/elm-csv-encode
RUN yes | /elm/elm install myrho/elm-round
# this just keeps the container awake so we can run other commands in it!
CMD ["sleep","1000"] 
