#!/usr/bin/env python3
import argparse
import asyncio
import signal
import sys

import aiohttp


def start_client(loop, url):
    auth = input('Enter your authentication details (ID:Token): ')

    headers={"Authorization": auth}
    ws = yield from aiohttp.ClientSession(headers=headers).ws_connect(url, autoclose=False, autoping=False)

    hub_channel = input('Enter the ID of the hub and channel messages should be sent in (hub_id:channel_id): ')

    asyncio.create_task(ws.send_str('SUBSCRIBE(' + hub_channel + ')'))

    def stdin_callback():
        line = sys.stdin.buffer.readline().decode('utf-8')
        if not line:
            loop.stop()
        else:
            asyncio.create_task(ws.send_str('SEND_MESSAGE(' + hub_channel + ',"' + line.strip() + '")'))
    loop.add_reader(sys.stdin.fileno(), stdin_callback)

    @asyncio.coroutine
    def dispatch():
        while True:
            msg = yield from ws.receive()

            if msg.type == aiohttp.WSMsgType.TEXT:
                print(msg.data.strip())
            elif msg.type == aiohttp.WSMsgType.BINARY:
                print('Binary: ', msg.data)
            elif msg.type == aiohttp.WSMsgType.PING:
                asyncio.create_task(ws.pong())
            else:
                if msg.type == aiohttp.WSMsgType.CLOSE:
                    yield from ws.close()
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    print('Error during receive %s' % ws.exception())
                elif msg.type == aiohttp.WSMsgType.CLOSED:
                    pass

                break

    yield from dispatch()


ARGS = argparse.ArgumentParser(
    description="websocket console client for wssrv.py example.")
ARGS.add_argument(
    '--host', action="store", dest='host',
    default='127.0.0.1', help='Host name')
ARGS.add_argument(
    '--port', action="store", dest='port',
    default=8080, type=int, help='Port number')

if __name__ == '__main__':
    args = ARGS.parse_args()
    if ':' in args.host:
        args.host, port = args.host.split(':', 1)
        args.port = int(port)

    url = 'http://{}:{}/v2/websocket'.format(args.host, args.port)

    loop = asyncio.get_event_loop()
    loop.add_signal_handler(signal.SIGINT, loop.stop)
    asyncio.Task(start_client(loop, url))
    loop.run_forever()