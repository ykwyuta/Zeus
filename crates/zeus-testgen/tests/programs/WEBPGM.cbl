       IDENTIFICATION DIVISION.
       PROGRAM-ID. WEBPGM.
      * An HTTP service: the request from WEB EXTRACT and WEB RECEIVE
      * (SET), storage from GETMAIN, a browse of URIMAPs and a loop of
      * deletes until a record is not found.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-METHOD             PIC X(8).
       01  WS-METHOD-LEN         PIC S9(8) COMP VALUE 8.
       01  WS-PATH               PIC X(32).
       01  WS-PATH-LEN           PIC S9(8) COMP VALUE 32.
       01  WS-BODY-PTR           POINTER.
       01  WS-BODY-LEN           PIC S9(8) COMP VALUE 80.
       01  WS-WORK-PTR           POINTER.
       01  WS-URIMAP             PIC X(8).
       01  WS-MAPS               PIC 9(3) VALUE 0.
       01  WS-SEGMENT            PIC 9(3) VALUE 0.
       01  WS-RESP               PIC S9(8) COMP.
       01  WS-KEY                PIC X(8).
       LINKAGE SECTION.
       01  BODY.
           05  BODY-CMD          PIC X(4).
       01  WORK-AREA             PIC X(100).
       PROCEDURE DIVISION.
           EXEC CICS WEB EXTRACT
                HTTPMETHOD(WS-METHOD) METHODLENGTH(WS-METHOD-LEN)
                PATH(WS-PATH) PATHLENGTH(WS-PATH-LEN)
                NOHANDLE
           END-EXEC
           IF WS-PATH-LEN = 0
               DISPLAY 'NO PATH'
           END-IF
           IF WS-METHOD = 'POST'
               EXEC CICS WEB RECEIVE SET(WS-BODY-PTR)
                    LENGTH(WS-BODY-LEN) MAXLENGTH(WS-BODY-LEN)
                    RESP(WS-RESP)
               END-EXEC
               SET ADDRESS OF BODY TO WS-BODY-PTR
               IF BODY-CMD = 'LOAD'
                   DISPLAY 'LOAD'
               END-IF
           END-IF
           EXEC CICS GETMAIN SET(WS-WORK-PTR) FLENGTH(100)
                INITIMG(X'40') NOHANDLE
           END-EXEC
           SET ADDRESS OF WORK-AREA TO WS-WORK-PTR
           MOVE 'WORK' TO WORK-AREA
           EXEC CICS INQUIRE URIMAP START NOHANDLE END-EXEC
           PERFORM WITH TEST AFTER UNTIL EIBRESP NOT = DFHRESP(NORMAL)
               EXEC CICS INQUIRE URIMAP(WS-URIMAP) NEXT NOHANDLE
               END-EXEC
               IF EIBRESP = DFHRESP(NORMAL)
                   ADD 1 TO WS-MAPS
               END-IF
           END-PERFORM
           EXEC CICS INQUIRE URIMAP END NOHANDLE END-EXEC
           PERFORM WITH TEST AFTER UNTIL EIBRESP NOT = DFHRESP(NORMAL)
               ADD 1 TO WS-SEGMENT
               EXEC CICS DELETE FILE('SEGMENTS') RIDFLD(WS-KEY)
                    NOHANDLE
               END-EXEC
           END-PERFORM
           IF WS-SEGMENT > 1
               DISPLAY 'SEGMENTS'
           END-IF
           EXEC CICS RETURN END-EXEC.
