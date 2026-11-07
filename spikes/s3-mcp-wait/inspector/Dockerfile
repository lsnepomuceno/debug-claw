# The MCP Inspector (the protocol's reference client), installed once so
# each CLI call does not download it again.
FROM node:22.23.3-alpine

RUN npm install -g @modelcontextprotocol/inspector@2.14.0 \
    && npm cache clean --force

ENTRYPOINT ["mcp-inspector"]
