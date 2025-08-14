        CREATE TABLE IF NOT EXISTS models (
            id                  INTEGER PRIMARY KEY,
            name                TEXT NOT NULL,
            name_short          TEXT NOT NULL,
            description         TEXT NOT NULL,
            comments            TEXT NOT NULL,
            num_of_times        INTEGER NOT NULL,
            num_of_times_short  INTEGER NOT NULL,
            score               INTEGER NOT NULL,
            score_short         INTEGER NOT NULL,
            image_name          TEXT NOT NULL,
            long_url            TEXT NOT NULL,
            short_url           TEXT NOT NULL
        );
            INSERT INTO models (id, name, name_short, description, comments, num_of_times, num_of_times_short, score, score_short, image_name, long_url, short_url)
            VALUES
              (1,'April01','April01Short','Side to side look at hands','',2,1,4,4,'April01.png', 'https://www.dropbox.com/scl/fi/jbp8cca0t9lmm1wh8pewx/April01.mp4?rlkey=rdwxc13kbmk2oihd1tb0i1ci1&dl=0', 'https://www.dropbox.com/scl/fi/me6tdolc7mt4zr7s4f4p1/April01Short.mp4?rlkey=2dfkcf10t3qfihfmrfclp5mqs&dl=0'),
              (2,'April04','April04Short','Flows focused on the White Crane','',3,4,4,3,'April04.png', 'https://www.dropbox.com/scl/fi/ppqw7r4amekpnxk3t5ouy/April04.mp4?rlkey=cz04vy5u0rwce9hg8nvtlk3p1&dl=0', 'https://www.dropbox.com/scl/fi/1nkv3brh8ruj3c87clfwt/April04Short.mp4?rlkey=97car8u968b1w0xkbyg7uirxi&dl=0'),
              (3,'April06','April06Short','Flows focused on Cloudy Hands and Centering','',2,1,4,4,'April06.png', 'https://www.dropbox.com/scl/fi/k3fd7ymnbxqw23v66cboj/April06.mp4?rlkey=xxviixzg4z4w9gs7gmf5jyrvi&dl=0', 'https://www.dropbox.com/scl/fi/qh2ygprv6dyvr8mbczoz2/April06Short.mp4?rlkey=cjv0he2vaw4e2kxjuoje7wehx&dl=0'),
              (4,'April08','April08Short','Bone breathing and flying in the flows','',3,4,4,3,'April08.png', 'https://www.dropbox.com/scl/fi/3cphx2fczf4mhsbgo7zk2/April08.mp4?rlkey=9wpd1szgbe322dpynzabjkm7n&dl=0', 'https://www.dropbox.com/scl/fi/ph4q5v4109mfbusuvu40a/April08Short.mp4?rlkey=y557s7mzejrbt3mqto3trygsv&dl=0'),
              (5,'April11','April11Short','Dragon Creates the Pearl','',3,4,4,3,'April11.png', 'https://www.dropbox.com/scl/fi/sj6fjdlk5u9tuo0jzxya1/April11.mp4?rlkey=rq9znrbe82tfal1e4rruo7394&dl=0', 'https://www.dropbox.com/scl/fi/89wbsznf58yluvcfjqags/April11Short.mp4?rlkey=tq6o2rzl3wnqlvso6jutevmlx&dl=0'),
              (6,'April13','April13Short','Flow focused on the earth element','',3,4,4,3,'April15.png', 'https://www.dropbox.com/scl/fi/bspzmtbfdn1kblt7ezne1/April13.mp4?rlkey=5foj1vemlvljimzjf4klorhn7&dl=0', 'https://www.dropbox.com/scl/fi/ws9p41n393evwi4kxyibw/April13Short.mp4?rlkey=nriwexi5r6fmz1cvg36m9dhea&dl=0'),
              (7,'April15','April15Short','Class focused on calming the mind','',3,4,4,3,'April15.png', 'https://www.dropbox.com/scl/fi/9fbrframbv56ylipko640/April15.mp4?rlkey=m1q3l4zgdk5n95r9eevc4rf65&dl=0', 'https://www.dropbox.com/scl/fi/ytmzcl1st2u3c2f1rc9tm/April15Short.mp4?rlkey=opzkmm021uuycend2jihaorao&dl=0'),
              (8,'April18','April18Short','Class focused on calming the mind','',3,4,4,3,'April15.png', 'https://www.dropbox.com/scl/fi/69q59lgtua47m8vtkhpt4/April18.mp4?rlkey=0okt86fyi5u4i7laqopabzhul&dl=0', 'https://www.dropbox.com/scl/fi/28rawekj4c24w7sxxbn50/April18Short.mp4?rlkey=psegrivuobdmup9lamxtzhy3h&dl=0')
            ON CONFLICT (id) DO NOTHING;
