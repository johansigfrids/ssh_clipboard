# SSH Clipboard

This tool is for copying clipboard content between machines on a local network. I has client implementations for Mac and Windows, and a server implementaiton for Linux. 

Hitting a keyboard shortcut copies to current content of the clipboard over to the server. A separate keyboard shortcut copies the content on the server over to the clipboard on the current client.

Communication with the server is done over SSH, and the server does not persist copied content to disk but keeps it in memory. 